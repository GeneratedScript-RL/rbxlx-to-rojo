use crate::{
    build_service_hierarchy, filesystem::FileSystem, process_instructions, sanitize_path_component,
    structures::*,
};
use log::info;
use pretty_assertions::assert_eq;
use rbx_dom_weak::{
    types::{Variant, Vector3},
    InstanceBuilder, WeakDom,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::ErrorKind,
    time::Instant,
};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
enum VirtualFileContents {
    Bytes(String),
    Instance(HashMap<String, Variant>),
    Vfs(VirtualFileSystem),
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct VirtualFile {
    contents: VirtualFileContents,
}

#[derive(Deserialize, Serialize, Debug, Default)]
struct VirtualFileSystem {
    files: BTreeMap<String, VirtualFile>,
    tree: BTreeMap<String, TreePartition>,
    #[serde(skip)]
    hierarchies: BTreeMap<String, ServiceHierarchy>,
    #[serde(skip)]
    finished: bool,
}

impl PartialEq<VirtualFileSystem> for VirtualFileSystem {
    fn eq(&self, rhs: &VirtualFileSystem) -> bool {
        self.files == rhs.files && self.tree == rhs.tree
    }
}

impl InstructionReader for VirtualFileSystem {
    fn finish_instructions(&mut self) {
        self.finished = true;
    }

    fn write_service_hierarchy(&mut self, hierarchy: &ServiceHierarchy) {
        self.hierarchies
            .insert(hierarchy.service.clone(), hierarchy.clone());
    }

    fn read_instruction<'a>(&mut self, instruction: Instruction<'a>) {
        match instruction {
            Instruction::AddToTree { name, partition } => {
                self.tree.insert(name, partition);
            }

            Instruction::CreateFile { filename, contents } => {
                let parent = filename
                    .parent()
                    .expect("no parent?")
                    .to_string_lossy()
                    .replace("\\", "/");
                let filename = filename
                    .file_name()
                    .expect("no filename?")
                    .to_string_lossy()
                    .replace("\\", "/");

                let system = if parent == "" {
                    self
                } else {
                    match self
                        .files
                        .get_mut(&parent)
                        .unwrap_or_else(|| panic!("no folder for {:?}", parent))
                        .contents
                    {
                        VirtualFileContents::Vfs(ref mut system) => system,
                        _ => unreachable!("attempt to parent to a file"),
                    }
                };

                let mut contents_string = String::from_utf8_lossy(&contents).into_owned();
                let rbxmx = filename.ends_with(".rbxmx");
                if filename.ends_with(".lua") {
                    contents_string = contents_string.replace("\r\n", "\n");
                }
                system.files.insert(
                    filename,
                    VirtualFile {
                        contents: if rbxmx {
                            let tree = rbx_xml::from_str_default(&contents_string)
                                .expect("couldn't decode encoded xml");
                            let child_id = tree.root().children()[0];
                            let child_instance = tree.get_by_ref(child_id).unwrap();
                            VirtualFileContents::Instance(
                                child_instance
                                    .properties
                                    .iter()
                                    .map(|(name, value)| (name.to_string(), value.clone()))
                                    .collect(),
                            )
                        } else {
                            VirtualFileContents::Bytes(contents_string)
                        },
                    },
                );
            }

            Instruction::CreateFolder { folder } => {
                let name = folder.to_string_lossy().replace("\\", "/");
                self.files.insert(
                    name,
                    VirtualFile {
                        contents: VirtualFileContents::Vfs(VirtualFileSystem::default()),
                    },
                );
            }
        }
    }
}

fn hierarchy_test_place() -> WeakDom {
    WeakDom::new(
        InstanceBuilder::new("DataModel")
            .with_child(
                InstanceBuilder::new("Workspace")
                    .with_property("Gravity", 196.2_f32)
                    .with_child(
                        InstanceBuilder::new("Part")
                            .with_name("Baseplate")
                            .with_property("Anchored", true)
                            .with_property("Size", Vector3::new(64.0, 1.0, 64.0)),
                    ),
            )
            .with_child(
                InstanceBuilder::new("StarterGui").with_child(
                    InstanceBuilder::new("ScreenGui")
                        .with_name("MainGui")
                        .with_property("Enabled", true),
                ),
            ),
    )
}

#[test]
fn sanitizes_roblox_names_for_windows_paths() {
    assert_eq!(
        sanitize_path_component("PlacementFROMPROD-6/17/2018"),
        "PlacementFROMPROD-6_17_2018"
    );
    assert_eq!(sanitize_path_component("CON"), "_CON");
    assert_eq!(sanitize_path_component("trailing. "), "trailing");
    assert_eq!(sanitize_path_component(".."), "_");
}

#[test]
fn exports_workspace_and_starter_gui_hierarchies() {
    let tree = hierarchy_test_place();
    let mut vfs = VirtualFileSystem::default();

    process_instructions(&tree, &mut vfs);

    let workspace = vfs.hierarchies.get("Workspace").unwrap();
    let workspace_root = workspace.root.as_ref().unwrap();
    assert_eq!(workspace.schema_version, 2);
    assert_eq!(workspace_root.class_name, "Workspace");
    assert_eq!(
        workspace_root.properties.get("Gravity"),
        Some(&Variant::Float32(196.2))
    );
    assert_eq!(workspace_root.children[0].name, "Baseplate");
    assert_eq!(
        workspace_root.children[0].properties.get("Anchored"),
        Some(&Variant::Bool(true))
    );

    let starter_gui = vfs.hierarchies.get("StarterGui").unwrap();
    let starter_gui_root = starter_gui.root.as_ref().unwrap();
    assert_eq!(starter_gui_root.children[0].name, "MainGui");
    assert_eq!(
        starter_gui_root.children[0].properties.get("Enabled"),
        Some(&Variant::Bool(true))
    );
}

#[test]
fn omits_internal_studio_properties_and_referents() {
    let tree = WeakDom::new(
        InstanceBuilder::new("DataModel").with_child(
            InstanceBuilder::new("Workspace")
                .with_property("HistoryId", true)
                .with_property("StudioDefaultStyleSheet", true)
                .with_property("StudioInsertWidgetLayerCollectorAutoLinkStyleSheet", true)
                .with_property("Gravity", 196.2_f32),
        ),
    );

    let hierarchy = build_service_hierarchy(&tree, "Workspace");
    let root = hierarchy.root.as_ref().unwrap();

    assert!(!root.properties.contains_key("HistoryId"));
    assert!(!root.properties.contains_key("StudioDefaultStyleSheet"));
    assert!(!root
        .properties
        .contains_key("StudioInsertWidgetLayerCollectorAutoLinkStyleSheet"));
    assert!(root.properties.contains_key("Gravity"));

    let json = serde_json::to_string(&hierarchy).unwrap();
    assert!(!json.contains("\"referent\""));
}

#[test]
fn reads_binary_places_with_hierarchy_properties() {
    let tree = hierarchy_test_place();
    let root_children = tree.root().children().to_vec();
    let mut encoded = Vec::new();

    rbx_binary::to_writer(&mut encoded, &tree, &root_children)
        .expect("couldn't encode binary place");
    let decoded =
        rbx_binary::from_reader(encoded.as_slice()).expect("couldn't decode binary place");

    let workspace = build_service_hierarchy(&decoded, "Workspace");
    let baseplate = &workspace.root.unwrap().children[0];
    assert_eq!(baseplate.name, "Baseplate");
    assert_eq!(
        baseplate.properties.get("Anchored"),
        Some(&Variant::Bool(true))
    );
}

#[test]
fn run_tests() {
    let _ = env_logger::init();
    for entry in fs::read_dir("./test-files").expect("couldn't read test-files") {
        let entry = entry.unwrap();
        let path = entry.path();
        info!("testing {:?}", path);

        let mut source_path = path.clone();
        source_path.push("source.rbxmx");
        let source = fs::read_to_string(&source_path).expect("couldn't read source.rbxmx");

        let time = Instant::now();
        let tree = rbx_xml::from_str_default(&source).expect("couldn't deserialize source.rbxmx");
        info!(
            "decoding for {:?} took {}ms",
            path,
            Instant::now().duration_since(time).as_millis()
        );

        let mut vfs = VirtualFileSystem::default();
        let time = Instant::now();
        process_instructions(&tree, &mut vfs);
        info!(
            "processing instructions for {:?} took {}ms",
            path,
            Instant::now().duration_since(time).as_millis()
        );

        let mut expected_path = path.clone();
        expected_path.push("output.json");
        assert!(vfs.finished, "finish_instructions was not called");

        if let Ok(expected) = fs::read_to_string(&expected_path) {
            assert_eq!(
                serde_json::from_str::<VirtualFileSystem>(&expected).unwrap(),
                vfs,
            );
        } else {
            let output = serde_json::to_string_pretty(&vfs).unwrap();
            fs::write(&expected_path, output).expect("couldn't write to output.json");
        }

        let filesystem_path = std::env::temp_dir()
            .join("rbxlx-to-rojo-tests")
            .join(path.file_name().unwrap());
        if let Err(error) = fs::remove_dir_all(&filesystem_path) {
            match error.kind() {
                ErrorKind::NotFound => {}
                other => panic!("couldn't remove filesystem dir: {:?}", other),
            }
        }

        fs::create_dir_all(&filesystem_path).unwrap();

        let mut filesystem = FileSystem::from_root(filesystem_path.clone());
        process_instructions(&tree, &mut filesystem);

        assert!(filesystem_path.join("workspace.json").is_file());
        assert!(filesystem_path.join("startergui.json").is_file());
    }
}
