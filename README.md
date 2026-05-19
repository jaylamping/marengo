# marengo

marengo/
├── Cargo.toml # workspace root, defines "armee"
├── Cargo.lock
├── README.md
├── .gitignore
├── rust-toolchain.toml
│
├── crates/ # Libraries
│ ├── armee-proto/ # Shared message types (the Chappe schema)
│ ├── armee-kinematics/ # FK/IK, joint limits — pure math, no hardware
│ ├── chappe/ # Message bus client (NATS/MQTT/custom)
│ ├── berthier/ # Motor control, owns CAN, RS03/Moteus drivers
│ │ └── src/
│ │ ├── lib.rs
│ │ ├── motor.rs
│ │ ├── arm.rs
│ │ └── config.rs
│ ├── davout/ # Safety supervisor (watchdog, limits, e-stop)
│ ├── talleyrand/ # Planner: intent → validated trajectory
│ ├── bulletin/ # Telemetry publisher
│ ├── fouche/ # Jetson-side LLM client + vision
│ └── robstride/ # Patched vendor crate
│
├── bins/ # Composed binaries
│ ├── marengo-pi/ # Pi runtime: berthier + davout + talleyrand + bulletin
│ ├── marengo-jetson/ # Jetson runtime: fouche
│ ├── probe/ # CAN diagnostic
│ ├── motor-repl/ # Interactive motor shell
│ └── wave-demo/ # V1 milestone binary
│
├── josephine (maybe?)/ # Frontend (Vite + React + TS), NOT in cargo workspace
│ ├── package.json
│ ├── vite.config.ts
│ └── src/
│
├── assets/
│ ├── urdf/marengo.urdf
│ └── meshes/
│ ├── visual/ # High-poly STL for Josephine viewer
│ └── collision/ # Decimated STL for sim
│
├── models/ # ONNX policies (probably git-lfs)
├── config/
│ ├── robot.yaml # Policy selection, joint config
│ └── network.yaml # Chappe broker addresses
│
├── docs/
│ ├── architecture.md
│ ├── decisions/ # ADRs (0001-no-ros2.md, 0002-chappe-bus.md, …)
│ └── bringup/ # Hardware notes
│
├── scripts/
│ ├── deploy-pi.sh
│ └── deploy-jetson.sh
│
└── .github/workflows/
├── ci.yml
└── deploy.yml
