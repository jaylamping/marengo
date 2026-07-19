# crates/talleyrand/

## Responsibility
**Motion planning** crate (in development). Future home for path planning, trajectory generation, and collision-aware motion for the humanoid.

## Design
- Placeholder crate in workspace; API not yet wired to Berthier control loop
- Named for Talleyrand (diplomat/planner) per Marengo codename convention

## Integration
- **Future consumer**: `bins/marengo-jetson`, high-level motion commands → Berthier

**Detailed map**: [src/codemap.md](src/codemap.md)
