# H7CAD — Geliştirme Yol Haritası

> Sürüm: 0.1.3 | Güncelleme: 2026-04-06

Durum simgeleri: ✅ Tamamlandı · 🔧 Kısmen yapıldı · ⬜ Yapılmadı

---

## 1. Dosya / Belge

| # | Özellik | Durum |
|---|---------|-------|
| 1.1 | DWG okuma (R13–R2018+) | ✅ |
| 1.2 | DXF okuma / yazma | ✅ |
| 1.3 | DWG yazma | ✅ |
| 1.4 | Otomatik format algılama (uzantıya göre) | ✅ |
| 1.5 | Çoklu sekme (tab) desteği | ✅ |
| 1.6 | Undo / Redo (snapshot stack) | ✅ |
| 1.7 | PDF dışa aktarma (CTB/STB plot style) | ✅ |
| 1.8 | Fiziksel yazıcıya yazdırma | ⬜ |
| 1.9 | XREF (dış referans) yönetimi | ✅ Auto-resolve on open, XATTACH/XREF/XRELOAD commands |
| 1.10 | WBLOCK — bloğu dış dosyaya yazma | ✅ Block name or selected entities → DWG/DXF |
| 1.11 | Serde entegrasyonu (JSON/alternatif I/O) | ⬜ |
| 1.12 | Bozuk DWG kurtarma (failsafe parse) | ⬜ |

---

## 2. Görselleştirme / Render

| # | Özellik | Durum |
|---|---------|-------|
| 2.1 | Wire (çizgi) render pipeline (GPU) | ✅ |
| 2.2 | Mesh (solid) render pipeline (GPU) | ✅ |
| 2.3 | Hatch (tarama) render pipeline (GPU) | ✅ |
| 2.4 | Raster image (PNG/JPG/BMP/TIFF) pipeline | ✅ |
| 2.5 | Wipeout maskeleme | ✅ |
| 2.6 | Wireframe / Hidden / Solid / X-Ray görünüm modları | ✅ |
| 2.7 | Karmaşık linetype (shape + dot + dash) render | ✅ |
| 2.8 | Arka plan rengi (BACKGROUND komutu) | ✅ |
| 2.9 | Çizim sırası (draw order / SortEntitiesTable) | ✅ |
| 2.10 | ViewCube (3D yönelim küpü) | ✅ |
| 2.11 | UCS simgesi (XYZ tripod) | ✅ |
| 2.12 | Solid3D / 3DSOLID tessellation (truck pipeline) | ✅ |
| 2.13 | Region / Body / Wire / Silhouette entity render | ⬜ |
| 2.14 | Anti-aliasing / MSAA seçeneği | ⬜ |

---

## 3. Entity Desteği (acadrust)

### 3.1 Tam Desteklenen Entity'ler
✅ Arc · Circle · Line · Ellipse · Spline · LwPolyline · Polyline (2D/3D) ·
Point · Solid (2D) · Ray · XLine · Face3D · Shape · Mesh ·
Text · MText · Attribute / AttDef · Leader · MultiLeader · Tolerance ·
Hatch · Dimension (Linear/Aligned/Angular/Diameter/Radius/Ordinate) ·
Insert (Block) · Table · MLine · Viewport · RasterImage · Wipeout ·
Underlay (PDF/DWF/DGN)

### 3.2 Kısmen / Sadece Okunabilir
| Entity | Durum |
|--------|-------|
| Solid3D (3DSOLID) | ✅ ACIS SAT tessellation |
| Region | ⬜ Tanınmıyor |
| Body / Wire / Silhouette | ⬜ Tanınmıyor |
| Ole2Frame | ⬜ Tanınmıyor |

### 3.3 XDATA (Genişletilmiş Veri)
✅ LIST / SET / CLEAR komutları tam entegre

---

## 4. Çizim (Draw) Komutları

| Komut | Durum |
|-------|-------|
| LINE (L) | ✅ |
| CIRCLE (C) | ✅ |
| ARC (A) | ✅ |
| ELLIPSE (EL) | ✅ |
| SPLINE (SPL) | ✅ |
| PLINE / LWPOLYLINE | ✅ |
| POLYLINE 3D | ✅ |
| POINT (PO) | ✅ |
| RAY | ✅ |
| XLINE (XL) | ✅ |
| HATCH (H) | ✅ |
| TEXT (DT) | ✅ |
| MTEXT (T) | ✅ |
| MLINE (ML) | ✅ |
| DONUT (DO) | ✅ |
| REVCLOUD | ✅ |
| WIPEOUT (WO) | ✅ |
| IMAGE (raster yerleştirme) | ✅ |
| SHAPE | ✅ |
| ATTDEF | ✅ |
| SOLID (2D dolu dörtgen) | ✅ |
| RECTANG (REC) | ✅ |
| POLYGON (POL) | ✅ |
| CONSTRUCTION LINE (tam sonsuz) | ✅ |

---

## 5. Modify (Düzenleme) Komutları

| Komut | Durum | Eksik Entity |
|-------|-------|--------------|
| MOVE (M) | ✅ | — |
| COPY (CO) | ✅ | — |
| ROTATE (RO) | ✅ | — |
| SCALE (SC) | ✅ | — |
| MIRROR (MI) | ✅ | — |
| DELETE / ERASE (E) | ✅ | — |
| ALIGN (AL) | ✅ | — |
| ARRAY Rectangular | ✅ | — |
| ARRAY Polar | ✅ | — |
| ARRAY Path | ✅ | — |
| BREAK (BR) | ✅ | — |
| TRIM (TR) | ✅ | — |
| EXTEND (EX) | ✅ | — |
| OFFSET (O) | ✅ | — |
| LENGTHEN (LEN) | ✅ | — |
| FILLET (F) | ✅ | — |
| CHAMFER (CHA) | ✅ | — |
| JOIN (J) | ✅ | — |
| EXPLODE (X) | ✅ | — |
| PEDIT (PE) | ✅ | — |
| STRETCH (SS) | ✅ | — |
| SPLINEDIT | ✅ CLOSE/OPEN/REVERSE + grip editing | — |
| HATCHEDIT | ✅ | — |
| ATTEDIT | ✅ Interactive tag-by-tag value editing | — |
| DDEDIT (çift tık metin) | ✅ | — |
| REFEDIT | ⬜ | Block in-place düzenleme |
| DIVIDE (DIV) | ✅ | — |
| MEASURE (ME) | ✅ | — |

---

## 6. Annotate (Ölçülendirme / Açıklama)

| Özellik | Durum |
|---------|-------|
| DIMLINEAR (DLI) | ✅ |
| DIMALIGNED (DAL) | ✅ |
| DIMANGULAR (DAN) | ✅ |
| DIMDIAMETER (DDI) | ✅ |
| DIMRADIUS (DRA) | ✅ |
| DIMBASELINE | ✅ |
| DIMCONTINUE | ✅ |
| LEADER (LE) | ✅ |
| MLEADER (MLD) | ✅ |
| TABLE | ✅ |
| TOLERANCE (GD&T) | ✅ |
| TEXT (DT) | ✅ |
| MTEXT (T) | ✅ |
| DIMSTYLE yöneticisi (DIMSTYLE/DDIM) | ✅ |
| MLEADERSTYLE | ⬜ |
| DIMORDINATE | ✅ |

---

## 7. Layer ve Stil Yönetimi

| Özellik | Durum |
|---------|-------|
| Layer Manager paneli | ✅ |
| LAYOFF / LAYON | ✅ |
| LAYFRZ / LAYTHW | ✅ |
| LAYLCK / LAYULK | ✅ |
| LAYISO / LAYUNISO | ✅ |
| Per-viewport layer freeze | ✅ |
| MATCHLAYER | ✅ |
| COLOR (renk atama) | ✅ |
| LINETYPE yönetimi | ✅ |
| LINEWEIGHT | ✅ |
| Transparency | ✅ |
| STYLE (TextStyle browser) | ✅ |
| DIMSTYLE | ✅ |
| MLSTYLE | ✅ |
| TABLESTYLE | ✅ |
| PLOTSTYLE (CTB/STB) | ✅ |
| Plot style arayüzü (GUI) | ⬜ |

---

## 8. Görünüm (View) ve Navigasyon

| Özellik | Durum |
|---------|-------|
| Pan (orta tuş / P) | ✅ |
| Zoom In / Out / Extent / All / Scale / Window | ✅ |
| Orbit (3D döndür) | ✅ |
| Perspektif / Ortografik geçiş | ✅ |
| Top / Front / Right / Isometric standart görünümler | ✅ |
| Plot Window önizleme | ✅ |
| UCS (WCS↔UCS dönüşüm pipeline) | ✅ |
| Named Views (VIEW komutu) | ✅ |
| Named UCS kaydetme | ✅ UCS SAVE/DELETE/LIST |
| VPORTS (viewport bölme) | ⬜ |
| Nesne snap izleme (Object Snap Tracking) | ✅ F11 toggle, dwell-acquire tracking lines |
| Dynamic Input overlay | ✅ F12 toggle, absolute XY + relative dist/angle |

---

## 9. Snap (Yakalama)

| Özellik | Durum |
|---------|-------|
| Endpoint / Midpoint / Center / Quadrant / Intersection | ✅ |
| Perpendicular / Tangent / Nearest | ✅ |
| Grid snap | ✅ Zoom-adaptive spacing |
| Polar tracking | ✅ Configurable angle, guide line |
| Object snap tracking | ✅ Dwell-acquire, H/V tracking lines, alignment snap |

---

## 10. Insert (Blok ve 3D Primitifler)

| Özellik | Durum |
|---------|-------|
| BLOCK (blok tanımlama) | ✅ |
| INSERT (blok yerleştirme) | ✅ |
| GROUP / UNGROUP | ✅ |
| Clipboard: COPY / CUT / PASTE / PASTEORIG | ✅ |
| 3D Box primitive | ✅ |
| 3D Sphere primitive | ✅ |
| 3D Cylinder primitive | ✅ |
| OBJ dosyası içe aktarma | ✅ |
| REFEDIT (block yerinde düzenleme) | ⬜ |
| WBLOCK (bloğu dış dosyaya yaz) | ✅ |
| Attributeli INSERT akışı (ATTREQ) | ✅ |

---

## 11. Layout ve Kağıt Alanı

| Özellik | Durum |
|---------|-------|
| Model Space / Paper Space ayrımı | ✅ |
| MVIEW (viewport oluşturma) | ✅ |
| Per-viewport layer visibility | ✅ |
| Plot ayarları (PlotSettings) | ✅ |
| Çoklu named layout sekmeleri | ✅ |
| VPLAYER — viewport katman override | 🔧 Altyapı var |
| Layout Manager arayüzü | ⬜ |

---

## 12. Inquiry (Sorgulama)

| Özellik | Durum |
|---------|-------|
| DIST — iki nokta arası mesafe | ✅ |
| ID — nokta koordinatı | ✅ |
| AREA — alan hesabı | ✅ |
| LIST — entity özellikleri | ✅ |
| FIND / FINDALL — metin ara/değiştir | ✅ |
| COUNT — entity istatistiği | ✅ |
| QSELECT — özelliğe göre seç | ✅ |
| FLATTEN (Z=0 düzleme) | ✅ |
| MASSPROP (alan merkezi, atalet) | ✅ |
| DATAEXTRACTION | ✅ CSV export: type/handle/layer/color/linetype/geometry |

---

## 13. UI / UX

| Özellik | Durum |
|---------|-------|
| Ribbon toolbar | ✅ |
| Command line | ✅ |
| Properties paneli | ✅ |
| Layer Manager paneli | ✅ |
| Status bar (viewport sayısı) | ✅ |
| Snap açılır popup | ✅ |
| Grip düzenleme (tüm entity tipleri) | ✅ |
| MATCHPROP (özellik kopyala) | ✅ |
| BYLAYER hızlı atama | ✅ |
| Çoklu seçim (window/crossing) | ✅ |
| Sağ tık bağlam menüsü | ✅ |
| Araç çubuğu özelleştirme | ⬜ |
| Tema / Renk şeması seçimi | ⬜ |
| Klavye kısayol düzenleyici | ⬜ |
| Komut geçmişi gezinme (↑↓) | ✅ |

---

## 14. 3D / Solid Modelleme

| Özellik | Durum |
|---------|-------|
| Truck geometry pipeline entegrasyonu | ✅ |
| 3D primitive'ler (Box, Sphere, Cylinder) | ✅ |
| OBJ mesh içe aktarma | ✅ |
| Solid3D tessellation (acadrust ACIS) | ✅ |
| Boolean operasyonlar (UNION/SUBTRACT/INTERSECT) | ⬜ |
| EXTRUDE / REVOLVE / SWEEP / LOFT | ⬜ |
| 3D ARRAY | ✅ |
| STL / STEP dışa aktarma | ⬜ |

---

## Öncelik Sırası (Bir Sonraki Adımlar)

### Yüksek Öncelik
1. **XREF yönetimi**

### Orta Öncelik
5. **MASSPROP / DATAEXTRACTION**

### Düşük Öncelik
11. Fiziksel yazıcıya yazdırma
12. Boolean 3D operasyonlar
