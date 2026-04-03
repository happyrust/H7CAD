# H7CAD — Roadmap

acadrust 0.3.1 referans alınarak hazırlanmıştır.

---

## Mevcut Durum (Özet)

### Entity Desteği

| Durum | Entity'ler |
|-------|-----------|
| **Tam** (render + grips + props + transform) | Point, Line, Circle, Arc, Ellipse, Spline, LwPolyline, Polyline, Polyline2D, Polyline3D, Ray, XLine, Text, MText, Leader, MultiLeader, RasterImage, Wipeout, AttributeDefinition, AttributeEntity, MLine, Table, Tolerance, Face3D, PolygonMesh, PolyfaceMesh |
| **Özel render, kısmi** | Dimension *(tessellate_dimension — grips yok)*, Hatch *(GPU HatchModel — grips yok)*, Insert *(explode_from_document)*, Viewport *(border render)* |
| **Hiç yok** | Solid *(2D dolu katı)*, Solid3D/Region/Body *(ACIS)*, Underlay *(PDF/DWF/DGN)*, Shape *(font glyph)* |

### I/O
- DWG okuma/yazma: R14 → R2018 (AC1032)
- DXF okuma/yazma
- PDF export (PlotSettings entegreli, rotation, offset, scale, window)

### Komutlar
203 benzersiz komut string — çizim, düzenleme, katman, görünüm, annotasyon, yardımcı.

### acadrust Tablo/Object Kullanımı
Kullanılan: `layers`, `linetypes`, `text_styles`, `dim_styles`, `views`, `ucss`, `vports`, `block_records`, `Layout`, `Group`, `PlotSettings`, `SortEntitiesTable`  
Kullanılmayan: `MLineStyle`, `TableStyle`, `ImageDefinition`

---

## Kalan İşler

### 1 — Entity Tamamlama

#### 1.1 Dimension Grips ✅
**Öncelik: Yüksek** — **TAMAMLANDI**

- [x] DimLinear: definition point, dim line, text grips
- [x] DimAligned: aynı
- [x] DimAngular: vertex + extension grips
- [x] DimRadius / DimDiameter: center + radius point grip
- [x] Grip ile dimension text konumu düzenleme

**Dosyalar:** `src/entities/dimension.rs`

---

#### 1.2 Hatch Boundary Grips ✅
**Öncelik: Orta** — **TAMAMLANDI**

- [x] Her boundary loop köşesi için grip (Polyline, Line, Arc, EllipticArc, Spline)
- [x] Grip sürükleme → boundary vertex güncelleme → hatch yeniden tessellate

**Dosyalar:** `src/entities/hatch.rs`, `src/scene/mod.rs`

---

#### 1.3 Solid Entity (2D) ✅
**Öncelik: Düşük** — **TAMAMLANDI**

- [x] `TruckConvertible`: wireframe çevre + GPU solid-fill (hatch pipeline)
- [x] Grips: 4 köşe
- [x] Properties: 4 köşe koordinatı, thickness
- [x] `Transformable`

**Dosyalar:** `src/entities/solid.rs`

---

#### 1.4 Underlay (PDF/DWF/DGN Referans)
**Öncelik: Düşük**

- [ ] Wireframe sınır kutusu render (RasterImage gibi)
- [ ] Fade/contrast/clip properties
- [ ] `UNDERLAY` komutu (dosya seçimi olmadan — sadece mevcut entity düzenleme)

**Dosyalar:** yeni `src/entities/underlay.rs`

---

### 2 — Viewport & Kağıt Uzayı

#### 2.1 VPCLIP
**Öncelik: Düşük** | **Bloker: acadrust `Viewport` struct'ında `clip_boundary` alanı yok**

- [ ] Özel overlay ile polygon clip sınırı çizimi
- [ ] VPCLIP komutu
- [ ] Clip boundary grip düzenlemesi

---

#### 2.2 Yeni Viewport Default Freeze Listesi
**Öncelik: Düşük** | **Bloker: acadrust VPLAYER NEWFRZ desteği yok**

- [ ] MVIEW ile oluşturulan yeni viewport'lara dondurulmuş layer listesi uygulanması

---

#### 2.3 Per-Viewport Arka Plan Rengi
**Öncelik: Düşük**

- [ ] Viewport entity'sine arka plan rengi alanı (custom veya overlay)
- [ ] wgpu pipeline'da per-viewport clear color

---

#### 2.4 Layer UI — Per-Viewport Görünürlük
**Öncelik: Düşük**

- [ ] Layer panelinde her viewport için ayrı freeze/thaw göstergesi

---

#### 2.5 Plot Style Table (CTB/STB)
**Öncelik: Orta**

- [ ] CTB (color-based) dosya okuma/yazma
- [ ] STB (named) dosya okuma/yazma
- [ ] PDF export sırasında plot style renk/kalınlık dönüşümü
- [ ] PlotSettings'e CTB/STB referansı

---

#### 2.6 PDF Shade Plot (Hidden Line Removal)
**Öncelik: Düşük**

- [ ] PDF export sırasında shade_plot_mode = Hidden → arka taraftaki kenarları gizle
- [ ] Depth sorting + backface culling (wireframe model için)

---

### 3 — Koordinat Sistemi

#### 3.1 WCS ↔ UCS Dönüşüm
**Öncelik: Orta**

Şu an tüm komutlar WCS (world) koordinatında çalışıyor.

- [ ] Aktif UCS matrisini komutlara entegre (giriş noktaları UCS → WCS dönüşümü)
- [ ] Snap grid'i UCS düzlemine hizala
- [ ] `UCS` komutu ile gerçek koordinat sistemi geçişi

**Dosyalar:** `src/scene/mod.rs`, `src/app/commands.rs`

---

#### 3.2 UCS Icon Render ✅
**Öncelik: Düşük** — **TAMAMLANDI**

- [x] Model space sol alt köşesinde XYZ eksen oku çizimi (iced canvas overlay)
- [x] Kamera rotasyonuna göre eksen yönleri dinamik olarak hesaplanıyor
- [x] `UCSICON` komutu ile toggle (ON/OFF)

---

### 4 — Style Yöneticileri (UI Dialogları)

#### 4.1 DimStyle Dialog
**Öncelik: Orta**

`DIMSTYLE LIST/NEW/SET` komutları mevcut; tam UI yok.

- [ ] Tüm DimStyle özelliklerini gösteren panel/dialog
- [ ] Ok tipi seçici (DIMBLK/DIMBLK1/DIMBLK2) — acadrust `dimblk` alanı var
- [ ] Tolerans format ayarları (DIMTOL, DIMLIM)
- [ ] Preview ile gerçek zamanlı önizleme

**Dosyalar:** yeni `src/ui/dimstyle_dialog.rs`

---

#### 4.2 TextStyle Font Browser
**Öncelik: Düşük**

`STYLE LIST/NEW/FONT/WIDTH/OBLIQUE` komutları mevcut; dosya browser yok.

- [ ] Sistem font listesi
- [ ] SHX font listesi (assets/fonts/)
- [ ] Önizleme metni

**Dosyalar:** yeni `src/ui/textstyle_dialog.rs`

---

#### 4.3 MLineStyle Yöneticisi
**Öncelik: Düşük**

`MLineStyle` acadrust'ta mevcut (`objects/mline_style.rs`) ama H7CAD hiç kullanmıyor.

- [ ] MLineStyle oluştur/düzenle (çizgi sayısı, offset, renk, linetype)
- [ ] Caps (başlangıç/bitiş çizgi, yay)
- [ ] MLINE komutuna stil seçimi entegre

**Dosyalar:** yeni `src/ui/mlinestyle_dialog.rs`

---

#### 4.4 TableStyle Yöneticisi
**Öncelik: Düşük**

`TableStyle` acadrust'ta mevcut (`objects/table_style.rs`) ama kullanılmıyor.

- [ ] Başlık satırı, veri satırı, sütun başlığı stilleri
- [ ] Font, renk, kenarlık, hizalama
- [ ] TABLE komutuna stil seçimi entegre

---

### 5 — Render İyileştirmeleri

#### 5.1 Wipeout Gerçek Maskeleme
**Öncelik: Düşük**

Şu an sadece wireframe sınır çiziliyor.

- [ ] SortEntitiesTable draw order entegrasyonu ile beyaz/arka plan renkli dolgu
- [ ] Polygon şeklini doldur (hatch solid benzeri)

---

#### 5.2 Tolerance GD&T Box Render
**Öncelik: Düşük**

Şu an sadece ham metin gösteriliyor.

- [ ] Gerçek feature control frame kutuları (bölmeli dikdörtgen)
- [ ] GD&T sembol seçici (Φ, ⊕, ○, □ vb.)

---

#### 5.3 IMAGE Komutu / Raster Pipeline
**Öncelik: Düşük**

RasterImage entity render yok (sadece sınır kutusu + X).

- [ ] PNG/JPEG/BMP decode (image crate)
- [ ] GPU texture upload + quad render
- [ ] `IMAGE` komutu: dosya seçimi dialog
- [ ] `ImageDefinition` object yönetimi

---

### 6 — 3D / ACIS

#### 6.1 Solid3D / Region / Body (ACIS)
**Öncelik: Çok Düşük** | **Bloker: SAT/SAB parser gerektirir**

acadrust `Solid3D`, `Region`, `Body` entity'lerini parse ediyor ama ACIS kernel yok.

- [ ] SAT text parse → B-rep half-edge yapısı
- [ ] Basit tessellation (flat shading, display-only)
- [ ] ACIS primitives: Box, Cylinder, Cone, Sphere, Torus

---

## Hızlı Referans — Kalan İşler Öncelik Sırası

| # | Özellik | Öncelik | Zorluk |
|---|---------|---------|--------|
| 1 | Dimension grips | Yüksek | Orta |
| 2 | WCS↔UCS dönüşüm | Orta | Yüksek |
| 3 | DimStyle dialog | Orta | Orta |
| 4 | Plot style table CTB/STB | Orta | Orta |
| 5 | Hatch boundary grips | Orta | Orta |
| 6 | Solid entity (2D) | Düşük | Kolay |
| 7 | Underlay entity | Düşük | Kolay |
| 8 | UCS icon render | Düşük | Kolay |
| 9 | Wipeout maskeleme | Düşük | Orta |
| 10 | MLineStyle yöneticisi | Düşük | Orta |
| 11 | TableStyle yöneticisi | Düşük | Orta |
| 12 | TextStyle font browser | Düşük | Kolay |
| 13 | IMAGE / raster pipeline | Düşük | Yüksek |
| 14 | Per-viewport arka plan rengi | Düşük | Yüksek |
| 15 | PDF shade plot | Düşük | Yüksek |
| 16 | VPCLIP | Düşük | Yüksek + Bloker |
| 17 | Solid3D / ACIS | Çok Düşük | Çok Yüksek |
