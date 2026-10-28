# Meshes

Visual and collision meshes exported from CAD (Brawner / `scripts/export-urdf.sh`).

Expected layout after export:

- `visual/` — render meshes for Consul and sim
- `collision/` — simplified collision for sim and future planning

Empty today: `assets/urdf/marengo.urdf` uses primitive shapes, which Consul and the sim render directly. Meshes arrive with the first mesh export ([docs/roadmap.md](../../docs/roadmap.md) M3).
