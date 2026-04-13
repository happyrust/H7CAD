# H7CAD — Feature Roadmap

Ribbon buttons that exist in the UI but have no backend implementation yet.
Each entry lists the command ID, what it should do, and rough complexity.

---

## Insert Tab

### Reference Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `XCLIP` | Clip | Define a clipping boundary on an inserted XREF or image block | Medium |
| `ADJUST` | Adjust | Adjust fade, contrast, and monochrome settings for an underlay | Low |
| `UNDERLAYLAYERS` | Underlay Layers | Show/hide individual layers inside a PDF/DWF underlay | Medium |
| `UOSNAP` | Snap to Underlays | Toggle object snap onto underlay geometry | Low |
| `FRAMES0/1/2` | Frames | Set underlay frame visibility (Off / On / On+Print) | Low |

### Point Cloud Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `POINTCLOUDATTACH` | Attach | Attach an RCP/RCS point cloud file as an external reference | High |

### Block Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `BLOCKPALETTE` | Multi-View Block | Open the block palette for inserting blocks with multiple views | Medium |
| `BEDIT` | Edit Block | Open the in-place block editor for the selected or named block | High |
| `BASE` | Set Base Point | Set the drawing base point (used as default 0,0,0 for XREF insertion) | Low |

### Attributes Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `ATTMAN` | Manage | Open attribute manager dialog (view/edit all attdefs in the drawing) | Medium |
| `ATTSYNC` | Synchronize | Synchronize attribute definitions across all INSERT instances of a block | Medium |

### Import Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `LANDXMLIMPORT` | Land XML | Import a LandXML file as survey/topo geometry | High |

### Content Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `CONTENTBROWSER` | Content Browser | Open Autodesk content browser for downloading blocks/materials | High |
| `ADCENTER` | Design Center | Open design center panel to browse/insert blocks from other drawings | High |

### Tables Group (Annotate Tab)

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `DATALINK` | Link Data | Create a data link between a table cell and an external Excel/CSV file | High |
| `DATAEXTRACTION` | Extract Data | Run the data extraction wizard to export attribute/property data | High |

---

## View Tab

### Viewport Tools Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `NAVVCUBE` | ViewCube | Toggle ViewCube display on/off in the viewport | Low |
| `NAVBAR` | Navigation Bar | Toggle the navigation bar (pan/zoom/orbit toolbar) on/off | Low |

### Model Viewports Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `VPJOIN` | Join | Join two adjacent viewports into one | Medium |
| `VPORTS_NAMED` | Named | Open the named viewports dialog to restore a saved viewport layout | Medium |
| `VPORTS_RESTORE` | Restore | Restore a previously saved named viewport configuration | Medium |

### Palettes Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `TOOLPALETTES` | Tool Palettes | Toggle the tool palettes panel | Medium |
| `PROPERTIES` | Properties | Toggle the properties panel showing selected entity properties | High |
| `SHEETSET` | Sheet Set Manager | Open the sheet set manager panel | High |

### Interface Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `FILETAB` | File Tabs | Toggle the file tab bar at the top of the drawing area | Low |
| `LAYOUTTAB` | Layout Tabs | Toggle the layout tab bar at the bottom of the drawing area | Low |
| `HORIZONTAL` | Tile Horizontally | Tile all open drawing windows horizontally | Low |
| `VERTICAL` | Tile Vertically | Tile all open drawing windows vertically | Low |
| `CASCADE` | Cascade | Cascade all open drawing windows | Low |

---

## Manage Tab

### Customization Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `CUI` | User Interface | Open the customize user interface editor (ribbon, toolbars, keybindings) | High |
| `CUIIMPORT` | Import | Import a partial CUI/CUIX customization file | Medium |
| `CUIEXPORT` | Export | Export the current customization to a CUI/CUIX file | Medium |
| `ALIASEDIT` | Edit Aliases | Open the command alias editor (acad.pgp equivalent) | Medium |
| `CUILOAD` | Load Partial CUI | Load an additional partial customization file at runtime | Medium |

### Cleanup Group

| Command | Label | What it should do | Complexity |
|---|---|---|---|
| `OVERKILL` | Overkill | Remove duplicate and overlapping geometry from the drawing | High |
| `AUDIT` | Audit | Check and repair drawing file integrity, report errors | High |
| `FINDNONPURGEABLE` | Find Non-Purgeable Items | List all items that cannot be purged (in-use blocks, styles, layers) | Medium |

---

## Complexity Key

- **Low** — a single flag/variable toggle or simple dialog, no geometry math
- **Medium** — involves UI interaction or document-level table edits
- **High** — requires new data structures, complex geometry, or external file I/O
