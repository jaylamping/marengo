# Marengo PDU schematic generators

The sheets in this folder are generated. Edit these scripts, not the `.kicad_sch` files.

| File | Makes |
|---|---|
| `gen.py` | Entry point: root and hierarchy, `marengo.kicad_sym`, `sym-lib-table` |
| `sheets.py`, `sheets2.py` | HP and LP sheet contents |
| `genfp.py` | `marengo.pretty` footprints; every dimension is cited in the footprint description |
| `parts/*.json` | Pin data for the custom symbols |

Regenerate and check, from `hardware/electrical/schematics/marengo_pdu`:

```bash
tools/regen.sh
```

That runs `gen/gen.py`, ERC, `tools/check.py` (netlist checks and pad audit) and `tools/wirescan.py` (wires that tie two labels). Pass a directory to write somewhere else, which is how to compare against the committed sheets. Pass: ERC 0 errors, `FAILS 0`, and no wirescan lines. Read the ERC warnings too; a `multiple_net_names` warning can be a real short.

Footprints are regenerated on their own:

```bash
python3 -I gen/genfp.py marengo.pretty
```

Needs macOS KiCad 10 at `/Applications/KiCad` and Python 3 with the standard library only. Output is deterministic: UUIDs come from uuid5 and the title date is fixed in `gen.py`, so an unchanged script gives identical bytes. Export the PDF and BOM after any change (`tools/bom.py` writes the BOM CSV from a netlist).
