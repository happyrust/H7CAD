use h7cad_native_model::*;
use std::fmt::{self, Write};

// ---------------------------------------------------------------------------
// DxfWriteError
// ---------------------------------------------------------------------------
//
// Structured error type for `write_dxf_strict`, paired with reader-side
// `DxfReadError`. Introduced in round 30 (2026-04-22 post-DIMALT roadmap,
// Phase 1) to replace the legacy `Result<String, String>` writer contract
// with a pattern-matchable enum while keeping the string-returning
// `write_dxf_string` / `write_dxf` as backward-compatible shims.

/// Errors emitted by `write_dxf_strict`.
///
/// Variants mirror the conceptual failure modes a DXF serialiser can hit.
/// The writer currently has no internal failure paths (all helpers are
/// infallible `fn(&mut DxfWriter, &CadDocument) -> ()`), so in practice
/// only a future callsite that validates document invariants or grows an
/// `&mut dyn Write` overload will actually surface these variants —— but
/// the enum is future-proofed **now** so downstream consumers can pattern
/// match instead of regex'ing strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxfWriteError {
    /// The document is internally inconsistent in a way that prevents
    /// serialisation (e.g. an IMAGE entity's `image_def_handle` points
    /// to a non-existent IMAGEDEF; a TABLE row references a vanished
    /// block record). Ship a human-readable description — callers that
    /// care about the precise shape should query the document directly.
    InvalidDocument(String),
    /// A serialisation feature is intentionally not implemented yet
    /// (e.g. binary DXF output, a specific R2018+ entity flavour).
    Unsupported(String),
    /// Low-level formatting failure surfaced from `std::fmt::Write`.
    /// The string-backed writer used by `write_dxf_strict` is infallible,
    /// so this variant is reserved for future `&mut dyn Write` overloads
    /// (streaming to a file handle / socket).
    Io(String),
}

impl fmt::Display for DxfWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDocument(msg) => write!(f, "invalid document: {msg}"),
            Self::Unsupported(msg) => write!(f, "unsupported: {msg}"),
            Self::Io(msg) => write!(f, "io: {msg}"),
        }
    }
}

impl std::error::Error for DxfWriteError {}

/// Wrap a free-form `String` error as `InvalidDocument` — a lossy but
/// convenient adaptor for callsites migrating from the legacy
/// `Result<String, String>` contract.
impl From<String> for DxfWriteError {
    fn from(msg: String) -> Self {
        Self::InvalidDocument(msg)
    }
}

impl From<&str> for DxfWriteError {
    fn from(msg: &str) -> Self {
        Self::InvalidDocument(msg.to_string())
    }
}

pub struct DxfWriter {
    buf: String,
}

impl DxfWriter {
    pub fn new() -> Self {
        Self {
            buf: String::with_capacity(64 * 1024),
        }
    }

    pub fn finish(self) -> String {
        self.buf
    }

    fn pair(&mut self, code: i16, value: &str) {
        writeln!(self.buf, "{:>3}", code).unwrap();
        writeln!(self.buf, "{}", value).unwrap();
    }

    fn pair_str(&mut self, code: i16, value: &str) {
        self.pair(code, value);
    }

    fn pair_i16(&mut self, code: i16, value: i16) {
        self.pair(code, &format!("{:>6}", value));
    }

    fn pair_i32(&mut self, code: i16, value: i32) {
        self.pair(code, &value.to_string());
    }

    fn pair_i64(&mut self, code: i16, value: i64) {
        self.pair(code, &value.to_string());
    }

    fn pair_f64(&mut self, code: i16, value: f64) {
        self.pair(code, &format_f64(value));
    }

    fn pair_handle(&mut self, code: i16, handle: Handle) {
        self.pair(code, &format!("{:X}", handle.value()));
    }

    fn point3d(&mut self, base_code: i16, p: [f64; 3]) {
        self.pair_f64(base_code, p[0]);
        self.pair_f64(base_code + 10, p[1]);
        self.pair_f64(base_code + 20, p[2]);
    }

    fn point2d(&mut self, base_code: i16, p: [f64; 2]) {
        self.pair_f64(base_code, p[0]);
        self.pair_f64(base_code + 10, p[1]);
    }
}

/// Stringify an `f64` for DXF output with **shortest round-trip
/// precision** plus a guaranteed decimal point.
///
/// Precision policy (escalated in the 2026-04-22 snap-grid-family plan,
/// round 25): `{:.10}` truncated transcendental constants like π/4 to
/// `0.7853981634`, silently losing ~7 digits of precision across a
/// read→write→read cycle. `f64::to_string()` uses Rust's shortest
/// round-trip algorithm (ryū-style) so `s.parse::<f64>()` always
/// recovers the identical bits.
///
/// Formatting policy (unchanged from the previous incarnation):
///
/// - `0.0` → `"0.0"` (canonicalised; avoids Rust's default `"-0"`).
/// - Any value whose shortest representation is a pure integer gets a
///   trailing `.0` so DXF consumers that parse on whitespace boundaries
///   still see a decimal point (`"1"` → `"1.0"`).
/// - Exponential forms (`"1e-20"`, `"1.5e10"`) are left untouched —
///   AutoCAD tokenises them as numbers too, and the `.0` cue only
///   matters for the fixed-decimal form.
fn format_f64(v: f64) -> String {
    if v == 0.0 {
        return "0.0".into();
    }
    let s = v.to_string();
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{s}.0")
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn write_dxf_strict(doc: &CadDocument) -> Result<String, DxfWriteError> {
    if needs_ensure_image_defs(doc) {
        let mut owned = doc.clone();
        ensure_image_defs(&mut owned);
        write_dxf_string_impl(&owned).map_err(DxfWriteError::from)
    } else {
        write_dxf_string_impl(doc).map_err(DxfWriteError::from)
    }
}

pub fn write_dxf_string(doc: &CadDocument) -> Result<String, String> {
    // Pre-pass: auto-create IMAGEDEF objects for any IMAGE entities that
    // arrived via UI / bridge with only an inline file_path and no handle
    // link. Done on a clone so the public API stays `&CadDocument`
    // (downstream `save_dxf` passes &NativeCadDocument by shared ref).
    if needs_ensure_image_defs(doc) {
        let mut owned = doc.clone();
        ensure_image_defs(&mut owned);
        write_dxf_string_impl(&owned)
    } else {
        write_dxf_string_impl(doc)
    }
}

fn write_dxf_string_impl(doc: &CadDocument) -> Result<String, String> {
    let mut w = DxfWriter::new();

    write_header(&mut w, doc);
    write_classes(&mut w, doc);
    write_tables(&mut w, doc);
    write_blocks(&mut w, doc);
    write_entities(&mut w, doc);
    write_objects(&mut w, doc);

    w.pair_str(0, "EOF");
    Ok(w.finish())
}

// ---------------------------------------------------------------------------
// IMAGEDEF auto-create pre-pass
// ---------------------------------------------------------------------------

/// Address of an IMAGE entity inside a CadDocument — either at the top
/// level (doc.entities) or nested in a block record's entity list.
enum ImageLoc {
    TopLevel(usize),
    Block(Handle, usize),
}

/// Scan pass: return true iff the document has at least one IMAGE entity
/// whose `image_def_handle == Handle::NULL` **and** whose `file_path` is
/// non-empty — that's the exact precondition for `ensure_image_defs` to
/// do any work. Used by `write_dxf_string` to avoid a gratuitous
/// `CadDocument::clone()` when the doc is already in standard form
/// (e.g. straight out of `read_dxf` on an AutoCAD-authored file).
fn needs_ensure_image_defs(doc: &CadDocument) -> bool {
    let is_pending = |e: &Entity| {
        matches!(
            &e.data,
            EntityData::Image {
                image_def_handle,
                file_path,
                ..
            } if *image_def_handle == Handle::NULL && !file_path.is_empty()
        )
    };
    doc.entities.iter().any(is_pending)
        || doc
            .block_records
            .values()
            .any(|br| br.entities.iter().any(is_pending))
}

/// For every IMAGE entity whose `image_def_handle == Handle::NULL` and
/// whose `file_path` is non-empty (i.e. an IMAGE constructed via bridge
/// / UI that never got linked to a proper IMAGEDEF object), allocate a
/// fresh handle, insert a matching `ObjectData::ImageDef` into
/// `doc.objects`, and backfill the handle onto the entity in place.
///
/// Three passes to dance around Rust's borrow rules:
///   1. Gather `(ImageLoc, file_path.clone(), image_size)` tuples while
///      holding only shared borrows on `doc.entities` and
///      `doc.block_records`.
///   2. With exclusive `&mut doc`, allocate handles + push IMAGEDEF
///      objects, recording the resulting `(ImageLoc, Handle)` pairs.
///   3. Walk the pairs and backfill `image_def_handle` on each IMAGE,
///      using the saved `ImageLoc` to index back into the correct
///      collection (top-level `doc.entities` vs. a specific block).
///
/// Idempotent: on a doc that already has all its IMAGEs linked, the
/// gather pass yields an empty `pending` vec and the function returns
/// without side effects.
fn ensure_image_defs(doc: &mut CadDocument) {
    let mut pending: Vec<(ImageLoc, String, [f64; 2])> = Vec::new();

    for (i, e) in doc.entities.iter().enumerate() {
        if let EntityData::Image {
            image_def_handle,
            file_path,
            image_size,
            ..
        } = &e.data
        {
            if *image_def_handle == Handle::NULL && !file_path.is_empty() {
                pending.push((ImageLoc::TopLevel(i), file_path.clone(), *image_size));
            }
        }
    }
    for (br_handle, br) in &doc.block_records {
        for (i, e) in br.entities.iter().enumerate() {
            if let EntityData::Image {
                image_def_handle,
                file_path,
                image_size,
                ..
            } = &e.data
            {
                if *image_def_handle == Handle::NULL && !file_path.is_empty() {
                    pending.push((
                        ImageLoc::Block(*br_handle, i),
                        file_path.clone(),
                        *image_size,
                    ));
                }
            }
        }
    }

    if pending.is_empty() {
        return;
    }

    let mut allocated: Vec<(ImageLoc, Handle)> = Vec::with_capacity(pending.len());
    for (loc, file_name, image_size) in pending {
        let new_handle = doc.allocate_handle();
        doc.objects.push(CadObject {
            handle: new_handle,
            owner_handle: Handle::NULL,
            data: ObjectData::ImageDef {
                file_name,
                image_size,
                // AutoCAD-spec defaults matching what `read_image_def`
                // returns when a legacy DXF omits the extension codes.
                pixel_size: [1.0, 1.0],
                class_version: 0,
                image_is_loaded: true,
                resolution_unit: 0,
            },
        });
        allocated.push((loc, new_handle));
    }

    for (loc, new_handle) in allocated {
        let ent_data = match loc {
            ImageLoc::TopLevel(i) => &mut doc.entities[i].data,
            ImageLoc::Block(br_handle, i) => {
                let br = doc
                    .block_records
                    .get_mut(&br_handle)
                    .expect("block_record handle disappeared between gather and backfill passes");
                &mut br.entities[i].data
            }
        };
        if let EntityData::Image {
            image_def_handle, ..
        } = ent_data
        {
            *image_def_handle = new_handle;
        }
    }
}

// ---------------------------------------------------------------------------
// HEADER
// ---------------------------------------------------------------------------

fn write_header(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "HEADER");

    w.pair_str(9, "$ACADVER");
    w.pair_str(1, doc.header.version.to_dxf());

    w.pair_str(9, "$INSBASE");
    w.point3d(10, doc.header.insbase);

    w.pair_str(9, "$EXTMIN");
    w.point3d(10, doc.header.extmin);

    w.pair_str(9, "$EXTMAX");
    w.point3d(10, doc.header.extmax);

    w.pair_str(9, "$LIMMIN");
    w.point2d(10, doc.header.limmin);

    w.pair_str(9, "$LIMMAX");
    w.point2d(10, doc.header.limmax);

    w.pair_str(9, "$LTSCALE");
    w.pair_f64(40, doc.header.ltscale);

    w.pair_str(9, "$TEXTSIZE");
    w.pair_f64(40, doc.header.textsize);

    w.pair_str(9, "$DIMSCALE");
    w.pair_f64(40, doc.header.dimscale);

    // ── Dimension defaults (Tier 1) ───────────────────────────────────────
    w.pair_str(9, "$DIMASZ");
    w.pair_f64(40, doc.header.dimasz);

    w.pair_str(9, "$DIMEXO");
    w.pair_f64(40, doc.header.dimexo);

    w.pair_str(9, "$DIMEXE");
    w.pair_f64(40, doc.header.dimexe);

    w.pair_str(9, "$DIMTXT");
    w.pair_f64(40, doc.header.dimtxt);

    w.pair_str(9, "$DIMGAP");
    w.pair_f64(40, doc.header.dimgap);

    w.pair_str(9, "$DIMTOFL");
    w.pair_i16(70, if doc.header.dimtofl { 1 } else { 0 });

    w.pair_str(9, "$DIMDEC");
    w.pair_i16(70, doc.header.dimdec);

    w.pair_str(9, "$DIMADEC");
    w.pair_i16(70, doc.header.dimadec);

    w.pair_str(9, "$DIMSTYLE");
    w.pair_str(2, &doc.header.dimstyle);

    w.pair_str(9, "$DIMTXSTY");
    w.pair_str(7, &doc.header.dimtxsty);

    // ── Tier-2 dim numerics ───────────────────────────────────────────────
    w.pair_str(9, "$DIMRND");
    w.pair_f64(40, doc.header.dimrnd);

    w.pair_str(9, "$DIMLFAC");
    w.pair_f64(40, doc.header.dimlfac);

    w.pair_str(9, "$DIMTDEC");
    w.pair_i16(70, doc.header.dimtdec);

    w.pair_str(9, "$DIMFRAC");
    w.pair_i16(70, doc.header.dimfrac);

    w.pair_str(9, "$DIMDSEP");
    w.pair_i16(70, doc.header.dimdsep);

    w.pair_str(9, "$DIMZIN");
    w.pair_i16(70, doc.header.dimzin);

    // ── Dimension alternate units (DIMALT*) ───────────────────────────────
    w.pair_str(9, "$DIMALT");
    w.pair_i16(70, doc.header.dim_alt);

    w.pair_str(9, "$DIMALTD");
    w.pair_i16(70, doc.header.dim_altd);

    w.pair_str(9, "$DIMALTF");
    w.pair_f64(40, doc.header.dim_altf);

    w.pair_str(9, "$DIMALTRND");
    w.pair_f64(40, doc.header.dim_altrnd);

    w.pair_str(9, "$DIMALTTD");
    w.pair_i16(70, doc.header.dim_alttd);

    w.pair_str(9, "$DIMALTTZ");
    w.pair_i16(70, doc.header.dim_alttz);

    w.pair_str(9, "$DIMALTU");
    w.pair_i16(70, doc.header.dim_altu);

    w.pair_str(9, "$DIMALTZ");
    w.pair_i16(70, doc.header.dim_altz);

    w.pair_str(9, "$DIMAPOST");
    w.pair_str(1, &doc.header.dim_apost);

    // ── Dimension arrow / symbol names ────────────────────────────────────
    w.pair_str(9, "$DIMBLK");
    w.pair_str(1, &doc.header.dim_blk);

    w.pair_str(9, "$DIMBLK1");
    w.pair_str(1, &doc.header.dim_blk1);

    w.pair_str(9, "$DIMBLK2");
    w.pair_str(1, &doc.header.dim_blk2);

    w.pair_str(9, "$DIMLDRBLK");
    w.pair_str(1, &doc.header.dim_ldrblk);

    w.pair_str(9, "$DIMARCSYM");
    w.pair_i16(70, doc.header.dim_arcsym);

    w.pair_str(9, "$DIMJOGANG");
    w.pair_f64(40, doc.header.dim_jogang);

    // ── Dimension visual control ──────────────────────────────────────────
    w.pair_str(9, "$DIMJUST");
    w.pair_i16(70, doc.header.dim_just);

    w.pair_str(9, "$DIMSD1");
    w.pair_i16(70, doc.header.dim_sd1);

    w.pair_str(9, "$DIMSD2");
    w.pair_i16(70, doc.header.dim_sd2);

    w.pair_str(9, "$DIMSE1");
    w.pair_i16(70, doc.header.dim_se1);

    w.pair_str(9, "$DIMSE2");
    w.pair_i16(70, doc.header.dim_se2);

    w.pair_str(9, "$DIMSOXD");
    w.pair_i16(70, doc.header.dim_soxd);

    w.pair_str(9, "$DIMATFIT");
    w.pair_i16(70, doc.header.dim_atfit);

    w.pair_str(9, "$DIMAZIN");
    w.pair_i16(70, doc.header.dim_azin);

    w.pair_str(9, "$DIMTIX");
    w.pair_i16(70, doc.header.dim_tix);

    // ── Dimension rendering attributes ────────────────────────────────────
    w.pair_str(9, "$DIMCLRD");
    w.pair_i16(70, doc.header.dim_clrd);

    w.pair_str(9, "$DIMCLRE");
    w.pair_i16(70, doc.header.dim_clre);

    w.pair_str(9, "$DIMCLRT");
    w.pair_i16(70, doc.header.dim_clrt);

    w.pair_str(9, "$DIMLWD");
    w.pair_i16(70, doc.header.dim_lwd);

    w.pair_str(9, "$DIMLWE");
    w.pair_i16(70, doc.header.dim_lwe);

    w.pair_str(9, "$DIMTAD");
    w.pair_i16(70, doc.header.dim_tad);

    w.pair_str(9, "$DIMTIH");
    w.pair_i16(70, doc.header.dim_tih);

    w.pair_str(9, "$DIMTOH");
    w.pair_i16(70, doc.header.dim_toh);

    w.pair_str(9, "$DIMDLE");
    w.pair_f64(40, doc.header.dim_dle);

    w.pair_str(9, "$DIMCEN");
    w.pair_f64(40, doc.header.dim_cen);

    w.pair_str(9, "$DIMTSZ");
    w.pair_f64(40, doc.header.dim_tsz);

    // ── Paper space control ───────────────────────────────────────────────
    w.pair_str(9, "$PSTYLEMODE");
    w.pair_i16(70, doc.header.pstylemode);

    w.pair_str(9, "$TILEMODE");
    w.pair_i16(70, doc.header.tilemode);

    w.pair_str(9, "$MAXACTVP");
    w.pair_i16(70, doc.header.maxactvp);

    w.pair_str(9, "$PSVPSCALE");
    w.pair_f64(40, doc.header.psvpscale);

    // ── Miscellaneous flags ───────────────────────────────────────────────
    w.pair_str(9, "$TREEDEPTH");
    w.pair_i16(70, doc.header.treedepth);

    w.pair_str(9, "$VISRETAIN");
    w.pair_i16(70, doc.header.visretain);

    w.pair_str(9, "$DELOBJ");
    w.pair_i16(70, doc.header.delobj);

    w.pair_str(9, "$PROXYGRAPHICS");
    w.pair_i16(70, doc.header.proxygraphics);

    // ── 3D Surface defaults ───────────────────────────────────────────────
    w.pair_str(9, "$SURFTAB1");
    w.pair_i16(70, doc.header.surftab1);

    w.pair_str(9, "$SURFTAB2");
    w.pair_i16(70, doc.header.surftab2);

    w.pair_str(9, "$SURFTYPE");
    w.pair_i16(70, doc.header.surftype);

    w.pair_str(9, "$SURFU");
    w.pair_i16(70, doc.header.surfu);

    w.pair_str(9, "$SURFV");
    w.pair_i16(70, doc.header.surfv);

    w.pair_str(9, "$PFACEVMAX");
    w.pair_i16(70, doc.header.pfacevmax);

    // ── Additional common variables ───────────────────────────────────────
    w.pair_str(9, "$MEASUREMENT");
    w.pair_i16(70, doc.header.measurement);

    w.pair_str(9, "$EXTNAMES");
    w.pair_i16(290, if doc.header.extnames { 1 } else { 0 });

    w.pair_str(9, "$WORLDVIEW");
    w.pair_i16(70, doc.header.worldview);

    w.pair_str(9, "$UNITMODE");
    w.pair_i16(70, doc.header.unitmode);

    w.pair_str(9, "$SPLMAXDEG");
    w.pair_i16(70, doc.header.splmaxdeg);

    // ── Paper space UCS ───────────────────────────────────────────────────
    w.pair_str(9, "$PUCSBASE");
    w.pair_str(2, &doc.header.pucsbase);

    w.pair_str(9, "$PUCSNAME");
    w.pair_str(2, &doc.header.pucsname);

    w.pair_str(9, "$PUCSORG");
    w.point3d(10, doc.header.pucsorg);

    w.pair_str(9, "$PUCSXDIR");
    w.point3d(10, doc.header.pucsxdir);

    w.pair_str(9, "$PUCSYDIR");
    w.point3d(10, doc.header.pucsydir);

    // ── Additional DIM controls ───────────────────────────────────────────
    w.pair_str(9, "$DIMPOST");
    w.pair_str(1, &doc.header.dim_post);

    w.pair_str(9, "$DIMLUNIT");
    w.pair_i16(70, doc.header.dim_lunit);

    // ── Object snap / selection ───────────────────────────────────────────
    w.pair_str(9, "$OSMODE");
    w.pair_i16(70, doc.header.osmode);

    w.pair_str(9, "$PICKSTYLE");
    w.pair_i16(70, doc.header.pickstyle);

    w.pair_str(9, "$LIMCHECK");
    w.pair_i16(70, doc.header.limcheck);

    // ── Rendering / display / metadata ────────────────────────────────────
    w.pair_str(9, "$PELEVATION");
    w.pair_f64(40, doc.header.pelevation);

    w.pair_str(9, "$FACETRES");
    w.pair_f64(40, doc.header.facetres);

    w.pair_str(9, "$ISOLINES");
    w.pair_i16(70, doc.header.isolines);

    w.pair_str(9, "$TEXTQLTY");
    w.pair_i16(70, doc.header.textqlty);

    w.pair_str(9, "$TSTACKALIGN");
    w.pair_i16(70, doc.header.tstackalign);

    w.pair_str(9, "$TSTACKSIZE");
    w.pair_i16(70, doc.header.tstacksize);

    w.pair_str(9, "$ACADMAINTVER");
    w.pair_i16(70, doc.header.acadmaintver);

    w.pair_str(9, "$CDATE");
    w.pair_f64(40, doc.header.cdate);

    w.pair_str(9, "$LASTSAVEDBY");
    w.pair_str(1, &doc.header.lastsavedby);

    w.pair_str(9, "$MENU");
    w.pair_str(1, &doc.header.menu);

    // ── Dimension tolerance ───────────────────────────────────────────────
    w.pair_str(9, "$DIMTP");
    w.pair_f64(40, doc.header.dim_tp);

    w.pair_str(9, "$DIMTM");
    w.pair_f64(40, doc.header.dim_tm);

    w.pair_str(9, "$DIMTOL");
    w.pair_i16(70, doc.header.dim_tol);

    w.pair_str(9, "$DIMLIM");
    w.pair_i16(70, doc.header.dim_lim);

    w.pair_str(9, "$DIMTVP");
    w.pair_f64(40, doc.header.dim_tvp);

    w.pair_str(9, "$DIMTFAC");
    w.pair_f64(40, doc.header.dim_tfac);

    w.pair_str(9, "$DIMTOLJ");
    w.pair_i16(70, doc.header.dim_tolj);

    // ── Additional UI / legacy ────────────────────────────────────────────
    w.pair_str(9, "$COORDS");
    w.pair_i16(70, doc.header.coords);

    w.pair_str(9, "$SPLTKNOTS");
    w.pair_i16(70, doc.header.spltknots);

    w.pair_str(9, "$BLIPMODE");
    w.pair_i16(70, doc.header.blipmode);

    // ── User variables ────────────────────────────────────────────────────
    w.pair_str(9, "$USERI1"); w.pair_i16(70, doc.header.useri1);
    w.pair_str(9, "$USERI2"); w.pair_i16(70, doc.header.useri2);
    w.pair_str(9, "$USERI3"); w.pair_i16(70, doc.header.useri3);
    w.pair_str(9, "$USERI4"); w.pair_i16(70, doc.header.useri4);
    w.pair_str(9, "$USERI5"); w.pair_i16(70, doc.header.useri5);
    w.pair_str(9, "$USERR1"); w.pair_f64(40, doc.header.userr1);
    w.pair_str(9, "$USERR2"); w.pair_f64(40, doc.header.userr2);
    w.pair_str(9, "$USERR3"); w.pair_f64(40, doc.header.userr3);
    w.pair_str(9, "$USERR4"); w.pair_f64(40, doc.header.userr4);
    w.pair_str(9, "$USERR5"); w.pair_f64(40, doc.header.userr5);

    // ── Geolocation / 3D walk / misc ──────────────────────────────────────
    w.pair_str(9, "$LATITUDE"); w.pair_f64(40, doc.header.latitude);
    w.pair_str(9, "$LONGITUDE"); w.pair_f64(40, doc.header.longitude);
    w.pair_str(9, "$TIMEZONE"); w.pair_i16(70, doc.header.timezone);
    w.pair_str(9, "$STEPSPERSEC"); w.pair_f64(40, doc.header.stepspersec);
    w.pair_str(9, "$STEPSIZE"); w.pair_f64(40, doc.header.stepsize);
    w.pair_str(9, "$LENSLENGTH"); w.pair_f64(40, doc.header.lenslength);
    w.pair_str(9, "$SKETCHINC"); w.pair_f64(40, doc.header.sketchinc);

    // ── Spline defaults ───────────────────────────────────────────────────
    w.pair_str(9, "$SPLFRAME");
    w.pair_i16(70, if doc.header.splframe { 1 } else { 0 });

    w.pair_str(9, "$SPLINETYPE");
    w.pair_i16(70, doc.header.splinetype);

    w.pair_str(9, "$SPLINESEGS");
    w.pair_i16(70, doc.header.splinesegs);

    // ── Multi-line (MLINE) defaults ───────────────────────────────────────
    w.pair_str(9, "$CMLSTYLE");
    w.pair_str(2, &doc.header.cmlstyle);

    w.pair_str(9, "$CMLJUST");
    w.pair_i16(70, doc.header.cmljust);

    w.pair_str(9, "$CMLSCALE");
    w.pair_f64(40, doc.header.cmlscale);

    // ── Insertion / display / edit miscellany ─────────────────────────────
    w.pair_str(9, "$INSUNITS");
    w.pair_i16(70, doc.header.insunits);

    w.pair_str(9, "$INSUNITSDEFSOURCE");
    w.pair_i16(70, doc.header.insunits_def_source);

    w.pair_str(9, "$INSUNITSDEFTARGET");
    w.pair_i16(70, doc.header.insunits_def_target);

    w.pair_str(9, "$LWDISPLAY");
    w.pair_i16(290, if doc.header.lwdisplay { 1 } else { 0 });

    w.pair_str(9, "$XEDIT");
    w.pair_i16(290, if doc.header.xedit { 1 } else { 0 });

    // ── Drawing identity and render metadata ──────────────────────────────
    w.pair_str(9, "$FINGERPRINTGUID");
    w.pair_str(2, &doc.header.fingerprint_guid);

    w.pair_str(9, "$VERSIONGUID");
    w.pair_str(2, &doc.header.version_guid);

    w.pair_str(9, "$DWGCODEPAGE");
    w.pair_str(3, &doc.header.dwg_codepage);

    w.pair_str(9, "$CSHADOW");
    w.pair_i16(280, doc.header.cshadow);

    w.pair_str(9, "$REQUIREDVERSIONS");
    w.pair_i64(160, doc.header.required_versions);

    // ── Drawing metadata addendum ─────────────────────────────────────────
    w.pair_str(9, "$PROJECTNAME");
    w.pair_str(1, &doc.header.project_name);

    w.pair_str(9, "$HYPERLINKBASE");
    w.pair_str(1, &doc.header.hyperlink_base);

    w.pair_str(9, "$INDEXCTL");
    w.pair_i16(70, doc.header.indexctl);

    w.pair_str(9, "$OLESTARTUP");
    w.pair_i16(290, if doc.header.olestartup { 1 } else { 0 });

    // ── Loft 3D defaults ──────────────────────────────────────────────────
    w.pair_str(9, "$LOFTANG1");
    w.pair_f64(40, doc.header.loft_ang1);

    w.pair_str(9, "$LOFTANG2");
    w.pair_f64(40, doc.header.loft_ang2);

    w.pair_str(9, "$LOFTMAG1");
    w.pair_f64(40, doc.header.loft_mag1);

    w.pair_str(9, "$LOFTMAG2");
    w.pair_f64(40, doc.header.loft_mag2);

    w.pair_str(9, "$LOFTNORMALS");
    w.pair_i16(70, doc.header.loft_normals);

    w.pair_str(9, "$LOFTPARAM");
    w.pair_i16(70, doc.header.loft_param);

    // ── Interactive geometry command defaults ─────────────────────────────
    w.pair_str(9, "$CHAMFERA");
    w.pair_f64(40, doc.header.chamfera);

    w.pair_str(9, "$CHAMFERB");
    w.pair_f64(40, doc.header.chamferb);

    w.pair_str(9, "$CHAMFERC");
    w.pair_f64(40, doc.header.chamferc);

    w.pair_str(9, "$CHAMFERD");
    w.pair_f64(40, doc.header.chamferd);

    w.pair_str(9, "$CHAMMODE");
    w.pair_i16(70, doc.header.chammode);

    w.pair_str(9, "$FILLETRAD");
    w.pair_f64(40, doc.header.filletrad);

    // ── 2.5-D default attachment ──────────────────────────────────────────
    w.pair_str(9, "$ELEVATION");
    w.pair_f64(40, doc.header.elevation);

    w.pair_str(9, "$THICKNESS");
    w.pair_f64(40, doc.header.thickness);

    w.pair_str(9, "$PDMODE");
    w.pair_i32(70, doc.header.pdmode);

    w.pair_str(9, "$PDSIZE");
    w.pair_f64(40, doc.header.pdsize);

    w.pair_str(9, "$LUNITS");
    w.pair_i16(70, doc.header.lunits);

    w.pair_str(9, "$LUPREC");
    w.pair_i16(70, doc.header.luprec);

    w.pair_str(9, "$AUNITS");
    w.pair_i16(70, doc.header.aunits);

    w.pair_str(9, "$AUPREC");
    w.pair_i16(70, doc.header.auprec);

    // ── Drawing mode flags ────────────────────────────────────────────────
    w.pair_str(9, "$ORTHOMODE");
    w.pair_i16(70, if doc.header.orthomode { 1 } else { 0 });

    w.pair_str(9, "$GRIDMODE");
    w.pair_i16(70, if doc.header.gridmode { 1 } else { 0 });

    w.pair_str(9, "$SNAPMODE");
    w.pair_i16(70, if doc.header.snapmode { 1 } else { 0 });

    w.pair_str(9, "$FILLMODE");
    w.pair_i16(70, if doc.header.fillmode { 1 } else { 0 });

    w.pair_str(9, "$MIRRTEXT");
    w.pair_i16(70, if doc.header.mirrtext { 1 } else { 0 });

    w.pair_str(9, "$ATTMODE");
    w.pair_i16(70, doc.header.attmode);

    // ── Snap & grid geometry (伴生 snapmode / gridmode / orthomode 布尔) ──
    w.pair_str(9, "$SNAPBASE");
    w.pair_f64(10, doc.header.snap_base[0]);
    w.pair_f64(20, doc.header.snap_base[1]);

    w.pair_str(9, "$SNAPUNIT");
    w.pair_f64(10, doc.header.snap_unit[0]);
    w.pair_f64(20, doc.header.snap_unit[1]);

    w.pair_str(9, "$SNAPSTYLE");
    w.pair_i16(70, doc.header.snap_style);

    w.pair_str(9, "$SNAPANG");
    w.pair_f64(50, doc.header.snap_ang);

    w.pair_str(9, "$SNAPISOPAIR");
    w.pair_i16(70, doc.header.snap_iso_pair);

    w.pair_str(9, "$GRIDUNIT");
    w.pair_f64(10, doc.header.grid_unit[0]);
    w.pair_f64(20, doc.header.grid_unit[1]);

    // ── Display & render flags ────────────────────────────────────────────
    w.pair_str(9, "$DISPSILH");
    w.pair_i16(70, doc.header.dispsilh);

    w.pair_str(9, "$DRAGMODE");
    w.pair_i16(70, doc.header.dragmode);

    w.pair_str(9, "$REGENMODE");
    w.pair_i16(70, doc.header.regenmode);

    w.pair_str(9, "$SHADEDGE");
    w.pair_i16(70, doc.header.shadedge);

    w.pair_str(9, "$SHADEDIF");
    w.pair_i16(70, doc.header.shadedif);

    // ── Current drawing attributes ────────────────────────────────────────
    w.pair_str(9, "$CLAYER");
    w.pair_str(8, &doc.header.clayer);

    w.pair_str(9, "$CECOLOR");
    w.pair_i16(62, doc.header.cecolor);

    w.pair_str(9, "$CELTYPE");
    w.pair_str(6, &doc.header.celtype);

    w.pair_str(9, "$CELWEIGHT");
    w.pair_i16(370, doc.header.celweight);

    w.pair_str(9, "$CELTSCALE");
    w.pair_f64(40, doc.header.celtscale);

    w.pair_str(9, "$CETRANSPARENCY");
    w.pair_i32(440, doc.header.cetransparency);

    // ── Angular conventions ───────────────────────────────────────────────
    w.pair_str(9, "$ANGBASE");
    w.pair_f64(50, doc.header.angbase);

    w.pair_str(9, "$ANGDIR");
    w.pair_i16(70, if doc.header.angdir { 1 } else { 0 });

    // ── Linetype-space scaling ────────────────────────────────────────────
    w.pair_str(9, "$PSLTSCALE");
    w.pair_i16(70, if doc.header.psltscale { 1 } else { 0 });

    // ── UCS (User Coordinate System) family ───────────────────────────────
    w.pair_str(9, "$UCSBASE");
    w.pair_str(2, &doc.header.ucsbase);

    w.pair_str(9, "$UCSNAME");
    w.pair_str(2, &doc.header.ucsname);

    w.pair_str(9, "$UCSORG");
    w.point3d(10, doc.header.ucsorg);

    w.pair_str(9, "$UCSXDIR");
    w.point3d(10, doc.header.ucsxdir);

    w.pair_str(9, "$UCSYDIR");
    w.point3d(10, doc.header.ucsydir);

    // ── Timestamp metadata ────────────────────────────────────────────────
    w.pair_str(9, "$TDCREATE");
    w.pair_f64(40, doc.header.tdcreate);

    w.pair_str(9, "$TDUPDATE");
    w.pair_f64(40, doc.header.tdupdate);

    w.pair_str(9, "$TDINDWG");
    w.pair_f64(40, doc.header.tdindwg);

    w.pair_str(9, "$TDUSRTIMER");
    w.pair_f64(40, doc.header.tdusrtimer);

    // ── Active-view metadata ──────────────────────────────────────────────
    w.pair_str(9, "$VIEWCTR");
    w.point2d(10, doc.header.viewctr);

    w.pair_str(9, "$VIEWSIZE");
    w.pair_f64(40, doc.header.viewsize);

    w.pair_str(9, "$VIEWDIR");
    w.point3d(10, doc.header.viewdir);

    w.pair_str(9, "$HANDSEED");
    w.pair_str(5, &format!("{:X}", doc.next_handle()));

    w.pair_str(0, "ENDSEC");
}

// ---------------------------------------------------------------------------
// CLASSES
// ---------------------------------------------------------------------------

fn write_classes(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "CLASSES");

    for cls in &doc.classes {
        w.pair_str(0, "CLASS");
        w.pair_str(1, &cls.dxf_name);
        w.pair_str(2, &cls.cpp_class_name);
        w.pair_str(3, &cls.application_name);
        w.pair_i32(90, cls.proxy_flags);
        w.pair_i32(91, cls.instance_count);
        w.pair_i16(
            280,
            if cls.was_a_proxy { 1 } else { 0 },
        );
        w.pair_i16(
            281,
            if cls.is_an_entity { 1 } else { 0 },
        );
    }

    w.pair_str(0, "ENDSEC");
}

// ---------------------------------------------------------------------------
// TABLES
// ---------------------------------------------------------------------------

fn write_tables(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "TABLES");

    write_vport_table(w, doc);
    write_ltype_table(w, doc);
    write_layer_table(w, doc);
    write_style_table(w, doc);
    write_dimstyle_table(w, doc);
    write_block_record_table(w, doc);

    w.pair_str(0, "ENDSEC");
}

fn write_vport_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "VPORT");
    w.pair_i16(70, doc.vports.len() as i16);

    for vp in doc.vports.values() {
        w.pair_str(0, "VPORT");
        if vp.handle != Handle::NULL {
            w.pair_handle(5, vp.handle);
        }
        w.pair_str(2, &vp.name);
        w.pair_i16(70, 0);
        w.point2d(10, vp.lower_left);
        w.point2d(11, vp.upper_right);
        w.point2d(12, vp.view_center);
        w.pair_f64(40, vp.view_height);
        w.pair_f64(41, vp.aspect_ratio);
        w.pair_f64(16, vp.view_direction[0]);
        w.pair_f64(26, vp.view_direction[1]);
        w.pair_f64(36, vp.view_direction[2]);
        w.pair_f64(17, vp.view_target[0]);
        w.pair_f64(27, vp.view_target[1]);
        w.pair_f64(37, vp.view_target[2]);
    }

    w.pair_str(0, "ENDTAB");
}

fn write_ltype_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "LTYPE");
    w.pair_i16(70, doc.linetypes.len() as i16);

    for lt in doc.linetypes.values() {
        w.pair_str(0, "LTYPE");
        if lt.handle != Handle::NULL {
            w.pair_handle(5, lt.handle);
        }
        w.pair_str(2, &lt.name);
        w.pair_i16(70, 0);
        w.pair_str(3, &lt.description);
        w.pair_i16(72, 65);
        w.pair_i16(73, lt.segments.len() as i16);
        w.pair_f64(40, lt.pattern_length);
        for seg in &lt.segments {
            w.pair_f64(49, seg.length);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_layer_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "LAYER");
    w.pair_i16(70, doc.layers.len() as i16);

    for layer in doc.layers.values() {
        w.pair_str(0, "LAYER");
        if layer.handle != Handle::NULL {
            w.pair_handle(5, layer.handle);
        }
        w.pair_str(2, &layer.name);
        let mut flags: i16 = 0;
        if layer.is_frozen {
            flags |= 1;
        }
        if layer.is_locked {
            flags |= 4;
        }
        w.pair_i16(70, flags);
        w.pair_i16(62, layer.color);
        w.pair_str(6, &layer.linetype_name);
        if layer.lineweight != -1 {
            w.pair_i16(370, layer.lineweight);
        }
        if !layer.plot {
            w.pair_i16(290, 0);
        }
        if layer.true_color != 0 {
            w.pair_i32(420, layer.true_color);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_style_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "STYLE");
    w.pair_i16(70, doc.text_styles.len() as i16);

    for ts in doc.text_styles.values() {
        w.pair_str(0, "STYLE");
        if ts.handle != Handle::NULL {
            w.pair_handle(5, ts.handle);
        }
        w.pair_str(2, &ts.name);
        w.pair_i16(70, ts.flags);
        w.pair_f64(40, ts.height);
        w.pair_f64(41, ts.width_factor);
        w.pair_f64(50, ts.oblique_angle);
        w.pair_str(3, &ts.font_name);
        if !ts.bigfont_name.is_empty() {
            w.pair_str(4, &ts.bigfont_name);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_dimstyle_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "DIMSTYLE");
    w.pair_i16(70, doc.dim_styles.len() as i16);

    for ds in doc.dim_styles.values() {
        w.pair_str(0, "DIMSTYLE");
        if ds.handle != Handle::NULL {
            w.pair_handle(105, ds.handle);
        }
        w.pair_str(2, &ds.name);
        w.pair_i16(70, 0);
        w.pair_f64(40, ds.dimscale);
        w.pair_f64(41, ds.dimasz);
        w.pair_f64(42, ds.dimexo);
        w.pair_f64(44, ds.dimgap);
        w.pair_f64(140, ds.dimtxt);
        w.pair_i16(271, ds.dimdec);
        w.pair_i16(277, ds.dimlunit);
        w.pair_i16(275, ds.dimaunit);
        if !ds.dimtxsty_name.is_empty() {
            w.pair_str(340, &ds.dimtxsty_name);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_block_record_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "BLOCK_RECORD");
    w.pair_i16(70, doc.block_records.len() as i16);

    for br in doc.block_records.values() {
        w.pair_str(0, "BLOCK_RECORD");
        w.pair_handle(5, br.handle);
        w.pair_str(2, &br.name);
    }

    w.pair_str(0, "ENDTAB");
}

// ---------------------------------------------------------------------------
// BLOCKS
// ---------------------------------------------------------------------------

fn write_blocks(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "BLOCKS");

    for br in doc.block_records.values() {
        w.pair_str(0, "BLOCK");
        if br.block_entity_handle != Handle::NULL {
            w.pair_handle(5, br.block_entity_handle);
        }
        w.pair_handle(330, br.handle);
        w.pair_str(2, &br.name);
        w.pair_i16(70, 0);
        // DXF code 10/20/30 — the block's base point (insertion anchor).
        // Previously hard-coded to the origin, which silently erased any
        // custom anchor on save. Write the actual stored base_point so
        // round-tripping a drawing with custom-anchored blocks is
        // lossless.
        w.point3d(10, br.base_point);

        for entity in &br.entities {
            write_entity(w, entity);
        }

        w.pair_str(0, "ENDBLK");
        if br.block_entity_handle != Handle::NULL {
            w.pair_handle(5, br.block_entity_handle);
        }
        w.pair_handle(330, br.handle);
    }

    w.pair_str(0, "ENDSEC");
}

// ---------------------------------------------------------------------------
// ENTITIES
// ---------------------------------------------------------------------------

fn write_entities(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "ENTITIES");

    for entity in &doc.entities {
        write_entity(w, entity);
    }

    w.pair_str(0, "ENDSEC");
}

fn write_entity(w: &mut DxfWriter, entity: &Entity) {
    let type_name = entity.data.type_name();
    w.pair_str(0, &type_name);

    if entity.handle != Handle::NULL {
        w.pair_handle(5, entity.handle);
    }
    if entity.owner_handle != Handle::NULL {
        w.pair_handle(330, entity.owner_handle);
    }

    w.pair_str(8, &entity.layer_name);
    if entity.color_index != 256 {
        w.pair_i16(62, entity.color_index);
    }
    if !entity.linetype_name.is_empty() {
        w.pair_str(6, &entity.linetype_name);
    }
    if (entity.linetype_scale - 1.0).abs() > 1e-12 {
        w.pair_f64(48, entity.linetype_scale);
    }
    if entity.lineweight != -1 {
        w.pair_i16(370, entity.lineweight);
    }
    if entity.true_color != 0 {
        w.pair_i32(420, entity.true_color);
    }
    if entity.invisible {
        w.pair_i16(60, 1);
    }
    if entity.transparency != 0 {
        w.pair_i32(440, entity.transparency);
    }
    if entity.thickness != 0.0 {
        w.pair_f64(39, entity.thickness);
    }
    if entity.extrusion != [0.0, 0.0, 1.0] {
        w.pair_f64(210, entity.extrusion[0]);
        w.pair_f64(220, entity.extrusion[1]);
        w.pair_f64(230, entity.extrusion[2]);
    }

    write_entity_data(w, entity);

    for (app, pairs) in &entity.xdata {
        w.pair_str(1001, app);
        for (code, val) in pairs {
            w.pair(*code, val);
        }
    }
}

fn write_entity_data(w: &mut DxfWriter, entity: &Entity) {
    match &entity.data {
        EntityData::Line { start, end } => {
            w.point3d(10, *start);
            w.point3d(11, *end);
        }
        EntityData::Circle { center, radius } => {
            w.point3d(10, *center);
            w.pair_f64(40, *radius);
        }
        EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            w.point3d(10, *center);
            w.pair_f64(40, *radius);
            w.pair_f64(50, *start_angle);
            w.pair_f64(51, *end_angle);
        }
        EntityData::Point { position } => {
            w.point3d(10, *position);
        }
        EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            w.point3d(10, *center);
            w.point3d(11, *major_axis);
            w.pair_f64(40, *ratio);
            w.pair_f64(41, *start_param);
            w.pair_f64(42, *end_param);
        }
        EntityData::LwPolyline {
            vertices,
            closed,
            constant_width,
        } => {
            w.pair_i32(90, vertices.len() as i32);
            w.pair_i16(70, if *closed { 1 } else { 0 });
            // Code 43 = LwPolyline constant width (only written when non-zero).
            if *constant_width != 0.0 {
                w.pair_f64(43, *constant_width);
            }
            for v in vertices {
                w.pair_f64(10, v.x);
                w.pair_f64(20, v.y);
                if v.start_width != 0.0 {
                    w.pair_f64(40, v.start_width);
                }
                if v.end_width != 0.0 {
                    w.pair_f64(41, v.end_width);
                }
                if v.bulge != 0.0 {
                    w.pair_f64(42, v.bulge);
                }
            }
        }
        EntityData::Text {
            insertion,
            height,
            value,
            rotation,
            style_name,
            width_factor,
            oblique_angle,
            horizontal_alignment,
            vertical_alignment,
            alignment_point,
        } => {
            w.point3d(10, *insertion);
            if let Some(point) = alignment_point {
                w.point3d(11, *point);
            }
            w.pair_f64(40, *height);
            if (*width_factor - 1.0).abs() > f64::EPSILON {
                w.pair_f64(41, *width_factor);
            }
            w.pair_str(1, value);
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
            if *oblique_angle != 0.0 {
                w.pair_f64(51, *oblique_angle);
            }
            if *horizontal_alignment != 0 {
                w.pair_i16(72, *horizontal_alignment);
            }
            if *vertical_alignment != 0 {
                w.pair_i16(73, *vertical_alignment);
            }
            if !style_name.is_empty() {
                w.pair_str(7, style_name);
            }
        }
        EntityData::MText {
            insertion,
            height,
            width,
            rectangle_height,
            value,
            rotation,
            style_name,
            attachment_point,
            line_spacing_factor,
            drawing_direction,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *height);
            w.pair_f64(41, *width);
            if let Some(rect_h) = rectangle_height {
                w.pair_f64(43, *rect_h);
            }
            if (*line_spacing_factor - 1.0).abs() > f64::EPSILON {
                w.pair_f64(44, *line_spacing_factor);
            }
            if *attachment_point != 0 {
                w.pair_i16(71, *attachment_point);
            }
            if *drawing_direction != 5 {
                w.pair_i16(72, *drawing_direction);
            }
            w.pair_str(1, value);
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
            if !style_name.is_empty() {
                w.pair_str(7, style_name);
            }
        }
        EntityData::Insert {
            block_name,
            insertion,
            scale,
            rotation,
            has_attribs,
            attribs,
        } => {
            w.pair_str(2, block_name);
            w.point3d(10, *insertion);
            if scale[0] != 1.0 {
                w.pair_f64(41, scale[0]);
            }
            if scale[1] != 1.0 {
                w.pair_f64(42, scale[1]);
            }
            if scale[2] != 1.0 {
                w.pair_f64(43, scale[2]);
            }
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
            if *has_attribs {
                w.pair_i16(66, 1);
            }
            for attrib in attribs {
                write_entity(w, attrib);
            }
            if *has_attribs {
                w.pair_str(0, "SEQEND");
                w.pair_str(8, &entity.layer_name);
            }
        }
        EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => {
            w.pair_str(2, pattern_name);
            w.pair_i16(70, if *solid_fill { 1 } else { 0 });
            w.pair_i32(91, boundary_paths.len() as i32);
            for path in boundary_paths {
                w.pair_i32(92, path.flags);
                if path.flags & 2 != 0 {
                    for edge in &path.edges {
                        write_hatch_edge(w, edge);
                    }
                } else if !path.edges.is_empty() {
                    w.pair_i32(93, path.edges.len() as i32);
                    for edge in &path.edges {
                        write_hatch_edge(w, edge);
                    }
                }
            }
        }
        EntityData::Dimension {
            dim_type,
            block_name,
            style_name,
            definition_point,
            text_midpoint,
            text_override,
            attachment_point,
            measurement,
            text_rotation,
            horizontal_direction,
            flip_arrow1,
            flip_arrow2,
            first_point,
            second_point,
            angle_vertex,
            dimension_arc,
            leader_length,
            rotation,
            ext_line_rotation,
        } => {
            w.pair_str(2, block_name);
            w.pair_str(3, style_name);
            w.point3d(10, *definition_point);
            w.point3d(11, *text_midpoint);
            w.pair_i16(70, *dim_type);
            if !text_override.is_empty() {
                w.pair_str(1, text_override);
            }
            if *attachment_point != 0 {
                w.pair_i16(71, *attachment_point);
            }
            if *measurement != 0.0 {
                w.pair_f64(42, *measurement);
            }
            if *text_rotation != 0.0 {
                w.pair_f64(53, *text_rotation);
            }
            if *horizontal_direction != 0.0 {
                w.pair_f64(51, *horizontal_direction);
            }
            if *flip_arrow1 {
                w.pair_i16(74, 1);
            }
            if *flip_arrow2 {
                w.pair_i16(75, 1);
            }
            w.point3d(13, *first_point);
            w.point3d(14, *second_point);
            if *angle_vertex != [0.0, 0.0, 0.0] {
                w.point3d(15, *angle_vertex);
            }
            if *dimension_arc != [0.0, 0.0, 0.0] {
                w.point3d(16, *dimension_arc);
            }
            if *leader_length != 0.0 {
                w.pair_f64(40, *leader_length);
            }
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
            if *ext_line_rotation != 0.0 {
                w.pair_f64(52, *ext_line_rotation);
            }
        }
        EntityData::Spline {
            degree,
            closed,
            knots,
            control_points,
            weights,
            fit_points,
            start_tangent,
            end_tangent,
        } => {
            let mut flags: i32 = 0;
            if *closed {
                flags |= 1;
            }
            w.pair_i32(70, flags);
            w.pair_i32(71, *degree);
            w.pair_i32(72, knots.len() as i32);
            w.pair_i32(73, control_points.len() as i32);
            w.pair_i32(74, fit_points.len() as i32);
            if *start_tangent != [0.0, 0.0, 0.0] {
                w.point3d(12, *start_tangent);
            }
            if *end_tangent != [0.0, 0.0, 0.0] {
                w.point3d(13, *end_tangent);
            }
            for k in knots {
                w.pair_f64(40, *k);
            }
            if !weights.is_empty() {
                for wt in weights {
                    w.pair_f64(41, *wt);
                }
            }
            for cp in control_points {
                w.point3d(10, *cp);
            }
            for fp in fit_points {
                w.point3d(11, *fp);
            }
        }
        EntityData::Face3D {
            corners,
            invisible_edges,
        } => {
            w.point3d(10, corners[0]);
            w.point3d(11, corners[1]);
            w.point3d(12, corners[2]);
            w.point3d(13, corners[3]);
            if *invisible_edges != 0 {
                w.pair_i16(70, *invisible_edges);
            }
        }
        EntityData::Solid {
            corners,
            normal,
            thickness,
        } => {
            w.point3d(10, corners[0]);
            w.point3d(11, corners[1]);
            w.point3d(12, corners[2]);
            w.point3d(13, corners[3]);
            if *thickness != 0.0 {
                w.pair_f64(39, *thickness);
            }
            if *normal != [0.0, 0.0, 1.0] {
                w.point3d(210, *normal);
            }
        }
        EntityData::Ray { origin, direction } | EntityData::XLine { origin, direction } => {
            w.point3d(10, *origin);
            w.point3d(11, *direction);
        }
        EntityData::Viewport {
            center,
            width,
            height,
        } => {
            w.point3d(10, *center);
            w.pair_f64(40, *width);
            w.pair_f64(41, *height);
        }
        EntityData::Polyline {
            polyline_type,
            vertices,
            closed,
        } => {
            let mut flags: i16 = 0;
            if *closed {
                flags |= 1;
            }
            match polyline_type {
                PolylineType::Polyline3D => flags |= 8,
                PolylineType::PolygonMesh => flags |= 16,
                PolylineType::PolyfaceMesh => flags |= 64,
                _ => {}
            }
            w.pair_i16(70, flags);
            for v in vertices {
                w.pair_str(0, "VERTEX");
                w.pair_str(8, &entity.layer_name);
                w.point3d(10, v.position);
                if v.start_width != 0.0 {
                    w.pair_f64(40, v.start_width);
                }
                if v.end_width != 0.0 {
                    w.pair_f64(41, v.end_width);
                }
                if v.bulge != 0.0 {
                    w.pair_f64(42, v.bulge);
                }
            }
            w.pair_str(0, "SEQEND");
            w.pair_str(8, &entity.layer_name);
        }
        EntityData::Attrib {
            tag,
            value,
            insertion,
            height,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *height);
            w.pair_str(1, value);
            w.pair_str(2, tag);
        }
        EntityData::AttDef {
            tag,
            prompt,
            default_value,
            insertion,
            height,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *height);
            w.pair_str(1, default_value);
            w.pair_str(2, tag);
            w.pair_str(3, prompt);
        }
        EntityData::Leader {
            vertices,
            has_arrowhead,
        } => {
            w.pair_i16(71, if *has_arrowhead { 1 } else { 0 });
            w.pair_i32(76, vertices.len() as i32);
            for v in vertices {
                w.point3d(10, *v);
            }
        }
        EntityData::MLine {
            vertices,
            style_name,
            scale,
            closed,
        } => {
            w.pair_str(2, style_name);
            w.pair_f64(40, *scale);
            // MLineFlags: HAS_VERTICES (1) + CLOSED (2 when closed).
            let flags: i16 = 1 | if *closed { 2 } else { 0 };
            w.pair_i16(71, flags);
            w.pair_i16(72, vertices.len() as i16);
            for v in vertices {
                w.point3d(11, *v);
            }
        }
        EntityData::Image {
            insertion,
            u_vector,
            v_vector,
            image_size,
            image_def_handle,
            file_path,
            display_flags,
        } => {
            w.point3d(10, *insertion);
            w.point3d(11, *u_vector);
            w.point3d(12, *v_vector);
            w.pair_f64(13, image_size[0]);
            w.pair_f64(23, image_size[1]);
            if *image_def_handle != Handle::NULL {
                // Standard DXF: code 340 hard-pointer to the linked
                // IMAGEDEF object. The IMAGEDEF object itself is emitted
                // from the OBJECTS section (see `write_object`) and owns
                // the authoritative file name via its own code 1.
                w.pair_handle(340, *image_def_handle);
            } else if !file_path.is_empty() {
                // Legacy H7CAD pre-D5 fallback: when the IMAGE entity has
                // no IMAGEDEF link (e.g. IMAGE was constructed directly by
                // H7CAD rather than parsed from a standard DXF), emit the
                // file path inline as code 1 so the native round-trip
                // survives even without a dedicated IMAGEDEF object.
                // parse_image recognises this form as a fallback.
                w.pair_str(1, file_path);
            }
            if *display_flags != 0 {
                w.pair_i32(70, *display_flags);
            }
        }
        EntityData::Wipeout {
            clip_vertices,
            elevation,
        } => {
            // Write insertion point (codes 10/20/30) — x/y zero, z = elevation.
            w.pair_f64(10, 0.0);
            w.pair_f64(20, 0.0);
            w.pair_f64(30, *elevation);
            w.pair_i32(91, clip_vertices.len() as i32);
            for v in clip_vertices {
                w.pair_f64(14, v[0]);
                w.pair_f64(24, v[1]);
            }
        }
        EntityData::Tolerance {
            text, insertion, ..
        } => {
            w.pair_str(1, text);
            w.point3d(10, *insertion);
        }
        EntityData::Shape {
            insertion,
            size,
            shape_number,
            name,
            rotation,
            relative_x_scale,
            oblique_angle,
            style_name,
            normal,
            thickness,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *size);
            w.pair_i16(2, *shape_number);
            if !name.is_empty() {
                w.pair_str(3, name);
            }
            if *thickness != 0.0 {
                w.pair_f64(39, *thickness);
            }
            if (*relative_x_scale - 1.0).abs() > f64::EPSILON {
                w.pair_f64(41, *relative_x_scale);
            }
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
            if *oblique_angle != 0.0 {
                w.pair_f64(51, *oblique_angle);
            }
            if *normal != [0.0, 0.0, 1.0] {
                w.point3d(210, *normal);
            }
            if !style_name.is_empty() {
                w.pair_str(7, style_name);
            }
        }
        EntityData::Solid3D { acis_data } | EntityData::Region { acis_data } => {
            for line in acis_data.lines() {
                w.pair_str(1, line);
            }
        }
        EntityData::MultiLeader {
            content_type,
            text_label,
            style_name,
            arrowhead_size,
            landing_gap,
            dogleg_length,
            property_override_flags,
            path_type,
            line_color,
            leader_line_weight,
            enable_landing,
            enable_dogleg,
            enable_annotation_scale,
            scale_factor,
            text_attachment_direction,
            text_bottom_attachment_type,
            text_top_attachment_type,
            text_location,
            leader_vertices,
            leader_root_lengths,
        } => {
            w.pair_i32(90, *property_override_flags as i32);
            w.pair_i16(170, *path_type);
            w.pair_i16(172, *content_type);
            w.pair_i32(91, *line_color);
            w.pair_i16(171, *leader_line_weight);
            w.pair_i16(290, if *enable_landing { 1 } else { 0 });
            w.pair_i16(291, if *enable_dogleg { 1 } else { 0 });
            w.pair_i16(293, if *enable_annotation_scale { 1 } else { 0 });
            if !text_label.is_empty() {
                w.pair_str(304, text_label);
            }
            if !style_name.is_empty() {
                w.pair_str(3, style_name);
            }
            w.pair_f64(41, *landing_gap);
            w.pair_f64(42, *arrowhead_size);
            w.pair_f64(43, *dogleg_length);
            w.pair_f64(45, *scale_factor);
            w.pair_i16(271, *text_attachment_direction);
            w.pair_i16(272, *text_bottom_attachment_type);
            w.pair_i16(273, *text_top_attachment_type);
            if let Some(loc) = text_location {
                w.pair_str(300, "CONTEXT_DATA{");
                w.pair_f64(12, loc[0]);
                w.pair_f64(22, loc[1]);
                w.pair_f64(32, loc[2]);
                w.pair_str(301, "}");
            }
            if !leader_vertices.is_empty() {
                let mut offset = 0usize;
                let lengths: Vec<usize> = if leader_root_lengths.is_empty() {
                    vec![leader_vertices.len()]
                } else {
                    leader_root_lengths.clone()
                };
                for len in lengths {
                    if len == 0 {
                        continue;
                    }
                    let end = (offset + len).min(leader_vertices.len());
                    if offset >= end {
                        break;
                    }
                    w.pair_str(302, "LEADER_LINE{");
                    for v in &leader_vertices[offset..end] {
                        w.pair_f64(10, v[0]);
                        w.pair_f64(20, v[1]);
                        w.pair_f64(30, v[2]);
                    }
                    w.pair_str(303, "}");
                    offset = end;
                }
                if offset < leader_vertices.len() {
                    w.pair_str(302, "LEADER_LINE{");
                    for v in &leader_vertices[offset..] {
                        w.pair_f64(10, v[0]);
                        w.pair_f64(20, v[1]);
                        w.pair_f64(30, v[2]);
                    }
                    w.pair_str(303, "}");
                }
            }
        }
        EntityData::Table {
            num_rows,
            num_cols,
            insertion,
            horizontal_direction,
            version,
            value_flag,
            row_heights,
            column_widths,
        } => {
            w.pair_i32(90, *value_flag);
            w.pair_i16(280, *version);
            w.pair_f64(10, insertion[0]);
            w.pair_f64(20, insertion[1]);
            w.pair_f64(30, insertion[2]);
            w.pair_f64(11, horizontal_direction[0]);
            w.pair_f64(21, horizontal_direction[1]);
            w.pair_f64(31, horizontal_direction[2]);
            w.pair_i32(91, *num_rows);
            w.pair_i32(92, *num_cols);
            for h in row_heights {
                w.pair_f64(141, *h);
            }
            for cw in column_widths {
                w.pair_f64(142, *cw);
            }
        }
        EntityData::Mesh {
            vertex_count,
            face_count,
            vertices,
            face_indices,
        } => {
            w.pair_i32(91, 0);
            w.pair_i32(92, *vertex_count);
            for v in vertices {
                w.point3d(10, *v);
            }
            w.pair_i32(93, *face_count);
            for idx in face_indices {
                w.pair_i32(90, *idx);
            }
        }
        EntityData::PdfUnderlay {
            insertion, scale, ..
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(41, scale[0]);
            w.pair_f64(42, scale[1]);
            w.pair_f64(43, scale[2]);
        }
        EntityData::Helix {
            axis_base_point,
            start_point,
            axis_vector,
            radius,
            turns,
            turn_height,
            handedness,
            is_ccw,
        } => {
            w.point3d(10, *axis_base_point);
            w.point3d(11, *start_point);
            w.point3d(12, *axis_vector);
            w.pair_f64(40, *radius);
            w.pair_f64(41, *turns);
            w.pair_f64(42, *turn_height);
            w.pair_i16(280, *handedness);
            w.pair_i16(290, if *is_ccw { 1 } else { 0 });
        }
        EntityData::ArcDimension {
            block_name,
            style_name,
            definition_point,
            text_midpoint,
            text_override,
            first_point,
            second_point,
            arc_center,
            leader_length,
            measurement,
        } => {
            w.pair_str(2, block_name);
            w.pair_str(3, style_name);
            w.point3d(10, *definition_point);
            w.point3d(11, *text_midpoint);
            w.pair_str(1, text_override);
            w.point3d(13, *first_point);
            w.point3d(14, *second_point);
            w.point3d(15, *arc_center);
            if *leader_length != 0.0 {
                w.pair_f64(40, *leader_length);
            }
            w.pair_f64(42, *measurement);
        }
        EntityData::LargeRadialDimension {
            block_name,
            style_name,
            definition_point,
            text_midpoint,
            text_override,
            chord_point,
            leader_length,
            jog_angle,
            measurement,
        } => {
            w.pair_str(2, block_name);
            w.pair_str(3, style_name);
            w.point3d(10, *definition_point);
            w.point3d(11, *text_midpoint);
            w.pair_str(1, text_override);
            w.point3d(15, *chord_point);
            w.pair_f64(40, *leader_length);
            w.pair_f64(50, *jog_angle);
            w.pair_f64(42, *measurement);
        }
        EntityData::Surface {
            u_isolines,
            v_isolines,
            acis_data,
            ..
        } => {
            w.pair_i32(70, *u_isolines);
            w.pair_i32(71, *v_isolines);
            for line in acis_data.lines() {
                w.pair_str(1, line);
            }
        }
        EntityData::Light {
            name,
            light_type,
            position,
            target,
            intensity,
            is_on,
            color,
            hotspot_angle,
            falloff_angle,
        } => {
            w.pair_str(1, name);
            w.pair_i16(70, *light_type);
            w.point3d(10, *position);
            w.point3d(11, *target);
            w.pair_f64(40, *intensity);
            w.pair_i16(290, if *is_on { 1 } else { 0 });
            w.pair_i16(63, *color);
            if *hotspot_angle != 0.0 {
                w.pair_f64(50, *hotspot_angle);
            }
            if *falloff_angle != 0.0 {
                w.pair_f64(51, *falloff_angle);
            }
        }
        EntityData::Camera {
            position,
            target,
            lens_length,
        } => {
            w.point3d(10, *position);
            w.point3d(11, *target);
            w.pair_f64(40, *lens_length);
        }
        EntityData::Section {
            name,
            state,
            vertices,
            vertical_direction,
        } => {
            w.pair_str(1, name);
            w.pair_i32(70, *state);
            w.pair_i32(90, vertices.len() as i32);
            for v in vertices {
                w.point3d(11, *v);
            }
            w.point3d(40, *vertical_direction);
        }
        EntityData::ProxyEntity {
            class_id,
            application_class_id,
            raw_codes,
        } => {
            w.pair_i32(90, *class_id);
            w.pair_i32(91, *application_class_id);
            for (code, val) in raw_codes {
                w.pair(*code, val);
            }
        }
        EntityData::Unknown { .. } => {
            // Cannot faithfully rewrite unknown entities
        }
    }
}

fn write_hatch_edge(w: &mut DxfWriter, edge: &HatchEdge) {
    match edge {
        HatchEdge::Line { start, end } => {
            w.pair_i16(72, 1);
            w.point2d(10, *start);
            w.point2d(11, *end);
        }
        HatchEdge::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            is_ccw,
        } => {
            w.pair_i16(72, 2);
            w.point2d(10, *center);
            w.pair_f64(40, *radius);
            w.pair_f64(50, *start_angle);
            w.pair_f64(51, *end_angle);
            w.pair_i16(73, if *is_ccw { 1 } else { 0 });
        }
        HatchEdge::EllipticArc {
            center,
            major_endpoint,
            minor_ratio,
            start_angle,
            end_angle,
            is_ccw,
        } => {
            w.pair_i16(72, 3);
            w.point2d(10, *center);
            w.point2d(11, *major_endpoint);
            w.pair_f64(40, *minor_ratio);
            w.pair_f64(50, *start_angle);
            w.pair_f64(51, *end_angle);
            w.pair_i16(73, if *is_ccw { 1 } else { 0 });
        }
        HatchEdge::Polyline { closed, vertices } => {
            let has_bulge = vertices.iter().any(|v| v[2] != 0.0);
            w.pair_i16(72, if has_bulge { 1 } else { 0 });
            w.pair_i16(73, if *closed { 1 } else { 0 });
            w.pair_i32(93, vertices.len() as i32);
            for v in vertices {
                w.pair_f64(10, v[0]);
                w.pair_f64(20, v[1]);
                if has_bulge {
                    w.pair_f64(42, v[2]);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OBJECTS
// ---------------------------------------------------------------------------

fn write_objects(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "OBJECTS");

    for obj in &doc.objects {
        write_object(w, obj);
    }

    w.pair_str(0, "ENDSEC");
}

fn write_object(w: &mut DxfWriter, obj: &CadObject) {
    match &obj.data {
        ObjectData::Dictionary { entries } => {
            w.pair_str(0, "DICTIONARY");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            for (name, handle) in entries {
                w.pair_str(3, name);
                w.pair_handle(350, *handle);
            }
        }
        ObjectData::XRecord { data_pairs } => {
            w.pair_str(0, "XRECORD");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            for (code, value) in data_pairs {
                w.pair(*code, value);
            }
        }
        ObjectData::Group {
            description,
            entity_handles,
        } => {
            w.pair_str(0, "GROUP");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(300, description);
            for h in entity_handles {
                w.pair_handle(340, *h);
            }
        }
        ObjectData::Layout {
            name,
            tab_order,
            block_record_handle,
            plot_paper_size,
            plot_origin,
        } => {
            w.pair_str(0, "LAYOUT");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, name);
            w.pair_i32(71, *tab_order);
            w.pair_handle(340, *block_record_handle);
            w.pair_f64(44, plot_paper_size[0]);
            w.pair_f64(45, plot_paper_size[1]);
            w.pair_f64(46, plot_origin[0]);
            w.pair_f64(47, plot_origin[1]);
        }
        ObjectData::DictionaryVar { schema, value } => {
            w.pair_str(0, "DICTIONARYVAR");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(280, schema);
            w.pair_str(1, value);
        }
        ObjectData::Scale {
            name,
            paper_units,
            drawing_units,
            is_unit_scale,
        } => {
            w.pair_str(0, "SCALE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(300, name);
            w.pair_f64(140, *paper_units);
            w.pair_f64(141, *drawing_units);
            w.pair_i16(290, if *is_unit_scale { 1 } else { 0 });
        }
        ObjectData::VisualStyle {
            description,
            style_type,
        } => {
            w.pair_str(0, "VISUALSTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, description);
            w.pair_i32(70, *style_type);
        }
        ObjectData::Material { name } => {
            w.pair_str(0, "MATERIAL");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, name);
        }
        ObjectData::ImageDef {
            file_name,
            image_size,
            pixel_size,
            class_version,
            image_is_loaded,
            resolution_unit,
        } => {
            w.pair_str(0, "IMAGEDEF");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, file_name);
            w.pair_f64(10, image_size[0]);
            w.pair_f64(20, image_size[1]);
            w.pair_f64(11, pixel_size[0]);
            w.pair_f64(21, pixel_size[1]);
            w.pair_i32(90, *class_version);
            w.pair_i16(71, if *image_is_loaded { 1 } else { 0 });
            w.pair_i16(281, *resolution_unit as i16);
        }
        ObjectData::ImageDefReactor { image_handle } => {
            w.pair_str(0, "IMAGEDEF_REACTOR");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_handle(330, *image_handle);
        }
        ObjectData::MLineStyle {
            name,
            description,
            element_count,
        } => {
            w.pair_str(0, "MLINESTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, name);
            w.pair_str(3, description);
            w.pair_i16(71, *element_count);
        }
        ObjectData::MLeaderStyle {
            name,
            content_type,
            text_style_handle,
        } => {
            w.pair_str(0, "MLEADERSTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, name);
            w.pair_i16(170, *content_type);
            w.pair_handle(340, *text_style_handle);
        }
        ObjectData::TableStyle {
            name, description, ..
        } => {
            w.pair_str(0, "TABLESTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, name);
            w.pair_str(3, description);
        }
        ObjectData::SortEntsTable {
            entity_handles,
            sort_handles,
        } => {
            w.pair_str(0, "SORTENTSTABLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            for (eh, sh) in entity_handles.iter().zip(sort_handles.iter()) {
                w.pair_handle(331, *eh);
                w.pair_handle(5, *sh);
            }
        }
        ObjectData::DimAssoc {
            associativity,
            dimension_handle,
        } => {
            w.pair_str(0, "DIMASSOC");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_i32(90, *associativity);
            w.pair_handle(330, *dimension_handle);
        }
        ObjectData::PlotSettings {
            page_name,
            printer_name,
            paper_size,
        } => {
            w.pair_str(0, "PLOTSETTINGS");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, page_name);
            w.pair_str(2, printer_name);
            w.pair_str(4, paper_size);
        }
        ObjectData::Field {
            evaluator_id,
            field_code,
        } => {
            w.pair_str(0, "FIELD");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, evaluator_id);
            w.pair_str(2, field_code);
        }
        ObjectData::IdBuffer { entity_handles } => {
            w.pair_str(0, "IDBUFFER");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            for h in entity_handles {
                w.pair_handle(330, *h);
            }
        }
        ObjectData::LayerFilter {
            name,
            layer_handles,
        } => {
            w.pair_str(0, "LAYER_FILTER");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, name);
            for h in layer_handles {
                w.pair_handle(8, *h);
            }
        }
        ObjectData::LightList {
            count,
            light_handles,
        } => {
            w.pair_str(0, "LIGHTLIST");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_i32(90, *count);
            for h in light_handles {
                w.pair_handle(5, *h);
            }
        }
        ObjectData::SunStudy {
            name,
            description,
            output_type,
        } => {
            w.pair_str(0, "SUNSTUDY");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, name);
            w.pair_str(2, description);
            w.pair_i16(70, *output_type);
        }
        ObjectData::DataTable {
            flags,
            column_count,
            row_count,
            name,
        } => {
            w.pair_str(0, "DATATABLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_i16(70, *flags);
            w.pair_i32(90, *column_count);
            w.pair_i32(91, *row_count);
            w.pair_str(1, name);
        }
        ObjectData::WipeoutVariables { frame_mode } => {
            w.pair_str(0, "WIPEOUTVARIABLES");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_i16(70, *frame_mode);
        }
        ObjectData::GeoData {
            coordinate_type,
            reference_point,
            design_point,
        } => {
            w.pair_str(0, "GEODATA");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_i16(70, *coordinate_type);
            w.point3d(10, *reference_point);
            w.point3d(11, *design_point);
        }
        ObjectData::RenderEnvironment {
            name,
            fog_enabled,
            fog_density_near,
            fog_density_far,
        } => {
            w.pair_str(0, "RENDERENVIRONMENT");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, name);
            w.pair_i16(290, if *fog_enabled { 1 } else { 0 });
            w.pair_f64(40, *fog_density_near);
            w.pair_f64(41, *fog_density_far);
        }
        ObjectData::ProxyObject {
            class_id,
            application_class_id,
            raw_codes,
        } => {
            w.pair_str(0, "ACAD_PROXY_OBJECT");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_i32(90, *class_id);
            w.pair_i32(91, *application_class_id);
            for (code, val) in raw_codes {
                w.pair(*code, val);
            }
        }
        ObjectData::Unknown { object_type } => {
            w.pair_str(0, object_type);
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_f64_trims_trailing_zeros() {
        assert_eq!(format_f64(0.0), "0.0");
        assert_eq!(format_f64(1.0), "1.0");
        assert_eq!(format_f64(1.5), "1.5");
        assert_eq!(format_f64(1.25), "1.25");
        assert_eq!(format_f64(-3.14), "-3.14");
    }

    #[test]
    fn write_minimal_document() {
        let doc = CadDocument::new();
        let output = write_dxf_string(&doc).unwrap();
        assert!(output.contains("SECTION"));
        assert!(output.contains("HEADER"));
        assert!(output.contains("$ACADVER"));
        assert!(output.contains("AC1015"));
        assert!(output.contains("ENDSEC"));
        assert!(output.contains("EOF"));
        assert!(output.contains("BLOCK_RECORD"));
        assert!(output.contains("*Model_Space"));
        assert!(output.contains("*Paper_Space"));
    }
}
