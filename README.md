# marengo

marengo/
├── Cargo.toml # workspace root, "armee"
├── README.md
├── .gitattributes # Git LFS rules
├── .gitignore
├── rust-toolchain.toml
│
├── hardware/ # PHYSICAL ROBOT — first-class citizen
│ ├── README.md # Hardware overview, current revision
│ ├── cad/
│ │ ├── parts/ # SolidWorks parts (.sldprt) — LFS
│ │ │ ├── shoulder/
│ │ │ │ ├── shoulder_housing.sldprt
│ │ │ │ ├── l_bracket_top.sldprt
│ │ │ │ └── …
│ │ │ ├── torso/
│ │ │ ├── arm/
│ │ │ └── head/
│ │ ├── assemblies/ # SolidWorks assemblies (.sldasm) — LFS
│ │ │ ├── marengo.sldasm # Top-level full robot
│ │ │ ├── shoulder.sldasm
│ │ │ └── arm.sldasm
│ │ ├── drawings/ # .slddrw + exported PDFs
│ │ └── vendor/ # Vendor-supplied CAD (.stp/.step) — LFS
│ │ ├── robstride/ # RS03.stp, RS01.stp
│ │ ├── moteus/
│ │ └── extrusions/ # 2020 profile, Mankk brackets
│ │
│ ├── electrical/
│ │ ├── pdb/ # Power Distribution Board v1.2
│ │ │ ├── schematic/ # KiCad schematic
│ │ │ ├── pcb/ # KiCad PCB layout
│ │ │ ├── gerbers/ # Fab outputs (or .gitignored, regenerated)
│ │ │ ├── bom.csv # PDB-specific BOM
│ │ │ └── README.md # Component groups, design notes
│ │ ├── wiring/
│ │ │ ├── harness.md # Wire gauges, lengths, runs
│ │ │ ├── can_topology.md # CAN1 (RS03) and CAN2 (Moteus) layout
│ │ │ └── connectors.md # XT30, JST, etc.
│ │ └── README.md
│ │
│ ├── prints/ # 3D print outputs and slicer notes
│ │ ├── stl/ # Source STLs ready for slicing
│ │ └── slicing.md # Material (PETG), infill, orientation per part
│ │
│ ├── bom/ # Overall BOM across mechanical + electrical
│ │ ├── master-bom.csv
│ │ └── vendor-sourcing.md # Multi-vendor options per part
│ │
│ └── docs/
│ ├── kinematics.md # Joint ranges, axes, transforms — single source of truth
│ ├── assembly.md # How to physically build it (this may never get updated/written)
│ └── decisions/ # Hardware ADRs (separate from software ones)
│
├── assets/ # DERIVED FROM hardware/, CONSUMED BY SOFTWARE
│ ├── urdf/
│ │ └── marengo.urdf # Exported via SW-to-URDF (Brawner's add-in)
│ └── meshes/
│ ├── visual/ # High-poly STL for Consul + sim display
│ └── collision/ # Decimated STL (MeshLab/Blender) for sim
│
├── crates/ # Rust libraries
│ ├── armee-proto/
│ ├── armee-kinematics/
│ ├── chappe/
│ ├── berthier/
│ ├── davout/
│ ├── talleyrand/
│ ├── fouche/
│ └── robstride/
│
├── bins/ # Rust binaries
│ ├── marengo-pi/
│ ├── marengo-jetson/
│ ├── probe/
│ ├── motor-repl/
│ └── wave-demo/
│
├── consul/ # Frontend (Vite + React + TS)
│
├── models/ # ONNX policies — Git LFS
├── config/ # Configuration files for the robot (Will actually probably want to implement in a db, i really don't like mutable yaml files)
│ ├── robot.yaml
│ └── network.yaml
│
├── docs/ # Software-level docs
│ ├── architecture.md
│ └── decisions/ # Software ADRs
│
├── scripts/
│ ├── deploy-pi.sh
│ ├── deploy-jetson.sh
│ └── export-urdf.sh # Runs SW exporter, decimates meshes, updates assets/
│
└── .github/workflows/
├── ci.yml
└── deploy.yml
