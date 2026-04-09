# User Testing

## Validation Surface

### File-level validation
- Run cargo test --workspace
- Use ACadSharp-led DXF fixtures first
- Expand to engineering-grade DXF samples in milestone 2
- Add minimal DWG fixtures in milestone 3

### Desktop UI smoke
- Launch H7CAD with cargo run
- Open a representative DXF or DWG sample through the real app flow
- Save or Save As the file
- Reopen the output and confirm tab title, path, dirty-state behavior, and visible scene

## Validation Concurrency
- File-level validation max concurrent validators: 5
- Desktop UI smoke max concurrent validators: 2

## Accepted Limitations
- Existing workspace skeleton is already complete and should be treated as baseline, not as new mission work
- Early desktop smoke may be partly manual until stable automation exists
