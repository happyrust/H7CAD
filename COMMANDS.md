# H7CAD — Command Reference

Status of every standard CAD command in H7CAD:

- ✅ **Implemented** — fully working
- 🔶 **Partial** — command is accepted but logic is a stub
- ❌ **Missing** — not yet implemented

---

## Draw

| Command | Alias | Description | Status |
|---|---|---|---|
| `LINE` | L | Straight line segment | ✅ |
| `PLINE` | PL | Polyline | ✅ |
| `ARC` | A | Arc | ✅ |
| `CIRCLE` | C | Circle | ✅ |
| `ELLIPSE` | EL | Ellipse | ✅ |
| `RECTANGLE` | REC | Rectangle | ✅ |
| `POLYGON` | POL | Regular polygon | ✅ |
| `XLINE` | XL | Infinite construction line | ✅ |
| `RAY` | — | One-way infinite line | ✅ |
| `SPLINE` | SPL | NURBS spline | ✅ |
| `MLINE` | ML | Multiline | ✅ |
| `POINT` | PO | Point | ✅ |
| `DONUT` | DO | Filled ring | ✅ |
| `HATCH` | H | Hatch fill | ✅ |
| `GRADIENT` | GD | Gradient fill | ✅ |
| `BOUNDARY` | BO | Boundary polyline / region | ✅ |
| `REVCLOUD` | — | Revision cloud | ✅ |
| `WIPEOUT` | WO | Wipeout mask | ✅ |
| `MTEXT` | MT | Multiline text | ✅ |
| `TEXT` | DT | Single-line text | ✅ |
| `TABLE` | — | Table entity | ✅ |
| `DIVIDE` | DIV | Divide entity into equal parts | ✅ |
| `MEASURE` | ME | Divide entity at measured intervals | ✅ |
| `3DPOLY` | — | 3D polyline | ❌ |
| `HELIX` | — | 3D helix | ❌ |
| `REGION` | REG | 2D closed region | ❌ |
| `TRACE` | — | Thick 2D line (legacy) | ❌ |
| `SKETCH` | — | Freehand sketch | ❌ |
| `SOLID` | SO | Filled 2D shape (legacy) | ❌ |
| `MINSERT` | — | Matrix block insert | ❌ |
| `FIELD` | — | Auto-updating text field | ❌ |

---

## Modify

| Command | Alias | Description | Status |
|---|---|---|---|
| `MOVE` | M | Move | ✅ |
| `COPY` | CO | Copy | ✅ |
| `ROTATE` | RO | Rotate | ✅ |
| `SCALE` | SC | Scale | ✅ |
| `MIRROR` | MI | Mirror | ✅ |
| `OFFSET` | O | Offset | ✅ |
| `TRIM` | TR | Trim | ✅ |
| `EXTEND` | EX | Extend | ✅ |
| `STRETCH` | S | Stretch | ✅ |
| `FILLET` | F | Fillet | ✅ |
| `CHAMFER` | CHA | Chamfer | ✅ |
| `ARRAY` | AR | Array | ✅ |
| `ARRAYRECT` | — | Rectangular array | ✅ |
| `ARRAYPOLAR` | — | Polar array | ✅ |
| `ARRAYPATH` | — | Path array | ✅ |
| `BREAK` | BR | Break entity | ✅ |
| `BREAKATPOINT` | — | Break at point | ✅ |
| `JOIN` | J | Join entities | ✅ |
| `EXPLODE` | X | Explode compound entity | ✅ |
| `ERASE` | E | Erase | ✅ |
| `LENGTHEN` | LEN | Lengthen / shorten | ✅ |
| `PEDIT` | PE | Edit polyline | ✅ |
| `SPLINEDIT` | SPE | Edit spline | ✅ |
| `MATCHPROP` | MA | Match properties | ✅ |
| `SCALETEXT` | — | Scale text objects | ✅ |
| `FLATTEN` | — | Flatten 3D to 2D | ✅ |
| `DRAWORDER` | DR | Draw order | ✅ |
| `ALIGN` | AL | Align | ✅ |
| `GROUP` | G | Group | ✅ |
| `UNGROUP` | UG | Ungroup | ✅ |
| `OVERKILL` | — | Remove duplicate geometry | 🔶 |
| `3DALIGN` | — | 3D align | ❌ |
| `3DMIRROR` | — | 3D mirror | ❌ |
| `3DMOVE` | — | 3D move | ❌ |
| `3DROTATE` | — | 3D rotate | ❌ |
| `3DARRAY` | — | 3D array | ❌ |
| `SLICE` | SL | Slice solid | ❌ |
| `SUBTRACT` | SU | Subtract solids | ❌ |
| `UNION` | UNI | Union solids | ❌ |
| `INTERSECT` | IN | Intersect solids | ❌ |
| `CHAMFERSOLID` | — | Chamfer solid edge | ❌ |
| `FILLETEDGE` | — | Fillet solid edge | ❌ |

---

## Dimension

| Command | Alias | Description | Status |
|---|---|---|---|
| `DIMLINEAR` | DLI | Linear dimension | ✅ |
| `DIMALIGNED` | DAL | Aligned dimension | ✅ |
| `DIMANGULAR` | DAN | Angular dimension | ✅ |
| `DIMRADIUS` | DRA | Radius dimension | ✅ |
| `DIMDIAMETER` | DDI | Diameter dimension | ✅ |
| `DIMORDINATE` | DOR | Ordinate dimension | ✅ |
| `DIMCONTINUE` | DCO | Continue dimension | ✅ |
| `DIMBASELINE` | DBA | Baseline dimension | ✅ |
| `QDIM` | — | Quick dimension | ✅ |
| `DIMEDIT` | DED | Edit dimension text / position | ✅ |
| `DIMTEDIT` | DIMTED | Move dimension text | ✅ |
| `DIMBREAK` | DBR | Break dimension line | ✅ |
| `DIMSPACE` | DSPACE | Adjust spacing between dimensions | ✅ |
| `DIMJOGLINE` | DJL | Jog line in dimension | ✅ |
| `TOLERANCE` | TOL | Geometric tolerance | ✅ |
| `LEADER` | LE | Leader line (legacy) | ✅ |
| `MLEADER` | MLD | Multileader | ✅ |
| `MLEADERADD` | MLA | Add leader segment | ✅ |
| `MLEADERREMOVE` | MLR | Remove leader segment | ✅ |
| `MLEADERALIGN` | MLAL | Align multileaders | ✅ |
| `MLEADERCOLLECT` | MLC | Collect multileaders | ✅ |
| `DIMJOGGED` | DJO | Jogged radius dimension | ❌ |
| `DIMCENTER` | DCE | Center mark | ❌ |
| `CENTERLINE` | — | Center line | ❌ |
| `CENTERMARK` | — | Center mark on arc/circle | ❌ |
| `QLEADER` | QL | Quick leader (legacy) | ❌ |

---

## Text & Table

| Command | Alias | Description | Status |
|---|---|---|---|
| `STYLE` | ST | Text style manager | ✅ |
| `DDEDIT` | ED | Edit text | ✅ |
| `FIND` | — | Find and replace text | ✅ |
| `TABLESTYLE` | TS | Table style manager | ✅ |
| `DATAEXTRACTION` | — | Data extraction wizard | 🔶 |
| `DATALINK` | — | Link table to external spreadsheet | 🔶 |
| `FIELD` | — | Auto-updating text field | ❌ |
| `SPELL` | SP | Spell check | ❌ |
| `ARCTEXT` | — | Text along an arc | ❌ |

---

## Layer

| Command | Alias | Description | Status |
|---|---|---|---|
| `LAYER` | LA | Layer manager | ✅ |
| `LAYOFF` | — | Turn layer off | ✅ |
| `LAYON` | — | Turn layer on | ✅ |
| `LAYFRZ` | — | Freeze layer | ✅ |
| `LAYTHW` | — | Thaw layer | ✅ |
| `LAYLCK` | — | Lock layer | ✅ |
| `LAYULK` | — | Unlock layer | ✅ |
| `LAYMCUR` | — | Make object's layer current | ✅ |
| `LAYMATCH` | — | Match layer of selected object | ✅ |
| `VPLAYER` | — | Viewport layer control | ✅ |
| `LINETYPE` | LT | Linetype manager | ✅ |
| `LTSCALE` | — | Global linetype scale | ✅ |
| `LAYISO` | — | Isolate layer | ❌ |
| `LAYUNISO` | — | End layer isolation | ❌ |
| `LAYWALK` | — | Walk through layers | ❌ |
| `LAYDEL` | — | Delete layer | ❌ |
| `LAYMRG` | — | Merge layers | ❌ |
| `LAYERSTATE` | — | Save / restore layer states | ❌ |
| `LAYLOCKFADECTL` | — | Locked layer fading control | ❌ |

---

## Block & Reference

| Command | Alias | Description | Status |
|---|---|---|---|
| `BLOCK` | B | Define block | ✅ |
| `INSERT` | I | Insert block | ✅ |
| `WBLOCK` | W | Write block to file | ✅ |
| `XATTACH` | XA | Attach external reference | ✅ |
| `XREF` | XR | External reference manager | ✅ |
| `XRELOAD` | — | Reload external reference | ✅ |
| `REFEDIT` | — | Edit reference in-place | ✅ |
| `REFCLOSE` | — | Close reference edit | ✅ |
| `ATTDEF` | ATT | Define attribute | ✅ |
| `ATTEDIT` | ATE | Edit attribute | ✅ |
| `ATTEXT` | — | Extract attributes (legacy) | ✅ |
| `XCLIP` | XC | Clip external reference | 🔶 |
| `BASE` | — | Set drawing base point | 🔶 |
| `BEDIT` | BE | Block editor | 🔶 |
| `BLOCKPALETTE` | — | Multi-view block palette | 🔶 |
| `ATTMAN` | — | Attribute manager | 🔶 |
| `ATTSYNC` | — | Synchronize attribute definitions | 🔶 |
| `MINSERT` | — | Matrix block insert | ❌ |
| `XBIND` | XB | Bind xref elements to drawing | ❌ |
| `XOPEN` | — | Open xref for editing | ❌ |
| `BSAVE` | — | Save block in editor | ❌ |
| `BCLOSE` | — | Close block editor | ❌ |

---

## PID & SPPID

| Command | Alias | Description | Status |
|---|---|---|---|
| `PIDSETDRAWNO` | — | Edit `SP_DRAWINGNUMBER` in PID Drawing XML | ✅ |
| `PIDSETPROP` | — | Edit arbitrary `SP_*` attribute in PID Drawing XML | ✅ |
| `PIDGETPROP` | — | Read current `SP_*` attribute from cached PID package | ✅ |
| `PIDSETGENERAL` | — | Edit `<element>text</element>` in PID General XML | ✅ |
| `SPPIDLOADLIB` | — | Seed the first-phase `SPPID_BRAN` authoring block library | ✅ |
| `SPPIDBRANDEMO` | — | Place one simple BRAN demo insert and guide lines | ✅ |
| `SPPIDEXPORT` | — | Export first-phase BRAN publish bundle: `.pid + _Data.xml + _Meta.xml` | ✅ |

---

## 3D Modeling

| Command | Alias | Description | Status |
|---|---|---|---|
| `BOX` | — | Box solid | ✅ |
| `SPHERE` | — | Sphere solid | ✅ |
| `CYLINDER` | — | Cylinder solid | ✅ |
| `EXTRUDE` | EXT | Extrude profile | ✅ |
| `REVOLVE` | REV | Revolve profile around axis | ✅ |
| `SWEEP` | — | Sweep profile along path | ✅ |
| `LOFT` | — | Loft between profiles | ✅ |
| `MASSPROP` | — | Mass properties | ✅ |
| `EXPORTSTEP` | — | Export to STEP | ✅ |
| `EXPORTSTL` | — | Export to STL | ✅ |
| `CONE` | — | Cone solid | ❌ |
| `PYRAMID` | — | Pyramid solid | ❌ |
| `WEDGE` | — | Wedge solid | ❌ |
| `TORUS` | — | Torus solid | ❌ |
| `HELIX` | — | 3D helix | ❌ |
| `POLYSOLID` | — | Wall-like solid | ❌ |
| `PRESSPULL` | — | Push / pull a face | ❌ |
| `THICKEN` | — | Thicken surface to solid | ❌ |
| `CONVTOSOLID` | — | Convert to solid | ❌ |
| `CONVTOSURFACE` | — | Convert to surface | ❌ |
| `SLICE` | SL | Slice solid with plane | ❌ |
| `SUBTRACT` | SU | Subtract solids | ❌ |
| `UNION` | UNI | Union solids | ❌ |
| `INTERSECT` | IN | Intersect solids | ❌ |
| `SECTION` | SEC | Section plane | ❌ |
| `SECTIONPLANE` | — | Section plane object | ❌ |
| `FLATSHOT` | — | 2D view from 3D | ❌ |
| `INTERFERE` | INF | Interference check | ❌ |

---

## View & Navigation

| Command | Alias | Description | Status |
|---|---|---|---|
| `ZOOM` | Z | Zoom | ✅ |
| `PAN` | P | Pan | ✅ |
| `ORBIT` | 3DO | 3D orbit | ✅ |
| `REGEN` | RE | Regenerate drawing | ✅ |
| `REDRAW` | R | Redraw viewport | ✅ |
| `VPORTS` | — | Viewport configuration | ✅ |
| `MSPACE` | MS | Switch to model space | ✅ |
| `PSPACE` | PS | Switch to paper space | ✅ |
| `MVIEW` | MV | Model view in layout | ✅ |
| `UCSICON` | — | Toggle UCS icon | ✅ |
| `NAVVCUBE` | — | Toggle ViewCube | 🔶 |
| `NAVBAR` | — | Toggle navigation bar | 🔶 |
| `VPJOIN` | — | Join viewports | 🔶 |
| `TOOLPALETTES` | — | Tool palettes panel | 🔶 |
| `PROPERTIES` | PR | Properties palette | 🔶 |
| `SHEETSET` | SSM | Sheet set manager | 🔶 |
| `FILETAB` | — | Toggle file tabs | 🔶 |
| `LAYOUTTAB` | — | Toggle layout tabs | 🔶 |
| `VIEW` | V | Named views manager | ❌ |
| `DVIEW` | DV | Dynamic view (legacy) | ❌ |
| `NAVSWHEEL` | — | Steering wheel | ❌ |
| `RENDER` | RR | Render | ❌ |
| `RENDERPRESETS` | — | Render presets | ❌ |
| `LIGHT` | — | Add scene light | ❌ |
| `SUNPROPERTIES` | — | Sun light settings | ❌ |
| `MATERIALS` | MAT | Material editor | ❌ |
| `VISUALSTYLES` | — | Visual style manager | ❌ |
| `HIDE` | HI | Hidden-line regeneration | ❌ |
| `PLAN` | — | Switch to plan view | ❌ |
| `VPMAX` | — | Maximize viewport | ❌ |
| `VPMIN` | — | Restore viewport | ❌ |

---

## Inquiry

| Command | Alias | Description | Status |
|---|---|---|---|
| `AREA` | — | Calculate area | ✅ |
| `MASSPROP` | — | Mass properties | ✅ |
| `QSELECT` | — | Quick select | ✅ |
| `STATUS` | — | Drawing status | ✅ |
| `COUNT` | — | Count objects | ✅ |
| `DIST` | DI | Distance between two points | ❌ |
| `ID` | — | Point coordinate | ❌ |
| `LIST` | LI | List object data | ❌ |
| `DBLIST` | — | List all objects | ❌ |
| `MEASUREGEOM` | — | Measure distance / angle / area | ❌ |
| `QUICKCALC` | QC | Quick calculator | ❌ |
| `CAL` | — | Command-line calculator | ❌ |

---

## File & Plot

| Command | Alias | Description | Status |
|---|---|---|---|
| `NEW` | — | New drawing | ✅ |
| `OPEN` | — | Open drawing | ✅ |
| `SAVE` | — | Save | ✅ |
| `SAVEAS` | — | Save as | ✅ |
| `QSAVE` | — | Quick save | ✅ |
| `PLOT` | — | Print / plot | ✅ |
| `EXPORT` | — | Export | ✅ |
| `PAGESETUP` | — | Page setup | ✅ |
| `PLOTSTYLE` | — | Plot style | ✅ |
| `PURGE` | PU | Purge unused items | ✅ |
| `EXPORTPDF` | — | Export to PDF | ❌ |
| `RECOVER` | — | Recover damaged drawing | ❌ |
| `CLOSE` | — | Close drawing | ❌ |
| `QUIT` | — | Exit application | ❌ |
| `ARCHIVE` | — | Archive drawing set | ❌ |
| `ETRANSMIT` | — | Transmit drawing package | ❌ |

---

## Manage & Customize

| Command | Alias | Description | Status |
|---|---|---|---|
| `RENAME` | — | Rename named objects | ✅ |
| `LINETYPE` | LT | Linetype manager | ✅ |
| `PLOTSTYLEEDITOR` | — | Plot style editor | ✅ |
| `MLEADERSTYLE` | — | Multileader style manager | ✅ |
| `DIMSTYLE` | D | Dimension style manager | ✅ |
| `AUDIT` | — | Audit drawing integrity | 🔶 |
| `OVERKILL` | — | Remove duplicate geometry | 🔶 |
| `CUI` | — | Customize user interface | 🔶 |
| `CUIIMPORT` | — | Import customization file | 🔶 |
| `CUIEXPORT` | — | Export customization file | 🔶 |
| `ALIASEDIT` | — | Edit command aliases | 🔶 |
| `FINDNONPURGEABLE` | — | Find non-purgeable items | 🔶 |
| `OPTIONS` | OP | Application settings | 🔶 |
| `XBIND` | — | Bind xref elements | ❌ |
| `HYPERLINK` | — | Insert hyperlink | ❌ |
| `DBCONNECT` | — | Connect to external database | ❌ |
| `SCRIPT` | SCR | Run script file | ❌ |
| `APPLOAD` | — | Load application (LISP / ARX) | ❌ |
| `NETLOAD` | — | Load .NET plug-in | ❌ |
| `ACTRECORD` | — | Record action macro | ❌ |
| `ACTMANAGER` | — | Action macro manager | ❌ |

---

## Summary

| Category | Total | ✅ Done | 🔶 Partial | ❌ Missing |
|---|---|---|---|---|
| Draw | 31 | 20 | 0 | 11 |
| Modify | 42 | 30 | 1 | 11 |
| Dimension | 25 | 22 | 0 | 3 |
| Text & Table | 9 | 6 | 2 | 1 |
| Layer | 19 | 12 | 0 | 7 |
| Block & Reference | 22 | 11 | 6 | 5 |
| 3D Modeling | 28 | 10 | 0 | 18 |
| View & Navigation | 31 | 10 | 8 | 13 |
| Inquiry | 12 | 5 | 0 | 7 |
| File & Plot | 16 | 10 | 0 | 6 |
| Manage & Customize | 21 | 5 | 7 | 9 |
| **Total** | **256** | **141** | **24** | **91** |
