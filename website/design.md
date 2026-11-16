# Desain website FluxCode

Spesifikasi `index.html`. Satu berkas HTML tanpa build step, gambar di `img/`.

Dial: ENERGY 2 / RHYTHM 3 / MOTION 2. Struktur mengikuti kiwstudio.com: judul rata kiri
dalam sentence case, panel data yang dibaca live dari sumbernya, satu warna aksen, tab bar
di bawah layar pada ponsel.

## Prinsip

- **Bukti, bukan klaim.** Setiap angka punya sumber: benchmark dari `tools/bench/` (waktu
  jendela FluxCode dari `tools/bench/manual.json`), angka repositori dan kontributor dari
  GitHub API saat halaman dibuka. Kalau sumbernya tidak ada, angkanya tidak ditampilkan.
- **Produk asli, bukan mockup.** Visual utama adalah tangkapan layar FluxCode dari
  `assets/screenshots/`, dikonversi ke WebP di `img/` (1600 px dan 800 px).
- **Unduh langsung.** Tombol unduh adalah `<a href download>` ke berkas rilis, jadi tetap
  bekerja tanpa JavaScript. Skrip hanya mengganti tautan ke build terbaru.
- **Jujur soal kekurangan.** Tabel perbandingan menulis yang belum dimiliki FluxCode
  (debugger, language server, kematangan).

## Warna

| Token | Nilai | Dipakai untuk |
| --- | --- | --- |
| `--bg` | `#0e0e10` | Latar halaman |
| `--surface`, `--surface-2` | `#151518`, `#1b1b1f` | Panel, baris unduhan terpilih |
| `--line`, `--line-strong` | `#24242a`, `#34343c` | Garis pemisah dan border |
| `--ink` | `#f2f0ea` | Teks utama |
| `--muted`, `--faint` | `#a3a09a`, `#8a877f` | Teks pendukung (kontras di atas 4,5:1 pada `--bg`) |
| `--yellow` | `#f0d55b` | Satu-satunya aksen: tombol utama, angka FluxCode, tab aktif |
| `--clay`, `--ivory` | `#b98a5e`, `#ebe4d6` | Dari logo meander: kait judul bagian, kode inline |

Batang pembanding (Zed, VS Code) abu-abu netral, bukan warna merek mereka, supaya kuning
hanya menandai FluxCode.

## Tipografi

- **BBH Bogle** (`--display`): judul, wordmark, dan angka besar. Hurufnya kapital semua
  (unicase) dan condensed, satu ketebalan, jadi selalu `font-weight: 400` dan
  `line-height` sekitar 1.
- **Schibsted Grotesk** (`--sans`): teks, tombol, label.
- **Spline Sans Mono** (`--mono`): hanya data teknis (tag build, nama berkas, ukuran, SHA commit).
- **Playwrite AU SA Guides** (`--hand`, kelas `.hand`): hanya untuk catatan tangan, saat ini
  dua tempat: "*diukur manual" di bawah angka ~50 ms dan ucapan terima kasih di bagian
  kontributor. Jangan dipakai untuk teks yang harus dibaca cepat.

## Motif

Kait meander di atas setiap judul bagian: garis yang berbelok tegak lurus, warna tanah liat
dari logo. Tidak ada grid latar, glow, atau blur.

## Urutan bagian

1. Header: logo, navigasi ke bagian yang ada, jumlah bintang live, tombol unduh sesuai OS.
2. Hero: judul, angka utama, tombol unduh, tag build dan waktu rilis live.
3. Penampil tangkapan layar: tujuh tab (Agen AI, Source Control, SQLite, Error Python,
   Go to File, Tema, Bahasa Indonesia), bisa dipakai dengan panah kiri dan kanan.
4. Benchmark: empat angka utama, lima grafik batang, cara mengukur di `<details>`.
5. Fitur: panel agen sebagai fitur utama, sepuluh fitur lain dalam dua kolom.
6. Perbandingan FluxCode, Zed, VS Code.
7. Unduh: empat baris (Windows, Windows portable, macOS, Linux); baris sistem pengunjung
   ditandai dan tombolnya kuning.
8. Dibangun terbuka: statistik repo, commit terbaru di `main`, kontributor.
9. Footer satu baris tautan.

## Gerak

- **Hero:** dua tangkapan layar asli (`agent` di depan, `themes` di belakang) jadi latar,
  miring dengan perspektif. Saat scroll, lapisan belakang bergerak 0,5x, lapisan depan 0,28x
  sambil pelan-pelan menegak, dan teks hero turun 0,18x sambil memudar. Gradien menjaga
  kontras teks.
- **Bagian lain:** judul bagian, screenshot, statistik, kartu agen, daftar unduhan, panel
  repo, dan kontributor bergeser maksimal 36 px berlawanan arah scroll (`DRIFT` di skrip).
- **Tab screenshot:** pill kuning bergeser ke tab yang dipilih; gambar memudar keluar,
  gambar baru di-decode dulu, lalu memudar masuk.
- **Ganti bahasa:** thumb EN/ID bergeser, teks memudar keluar lalu masuk dalam bahasa baru.
- **Muncul:** blok naik 16 px dan memudar masuk sekali saat masuk layar (jeda antar
  saudara 60 ms, maksimal 300 ms); batang grafik tumbuh dari nol. Memakai properti
  `translate`, jadi tidak bentrok dengan `transform` parallax.
- **Ringan di ponsel:** di layar sempit, mode hemat data, atau CPU 4 inti ke bawah, parallax
  hanya di hero dan judul bagian. Semua posisi dibaca dulu lalu ditulis sekaligus per frame.
- Hanya `transform`, `translate`, dan `opacity`, satu `requestAnimationFrame` per frame.
  `prefers-reduced-motion` mematikan semuanya.

## Panel repositori

Tiga tingkat, bukan enam kotak sama besar: jumlah bintang besar dengan tombol Star; total
unduhan installer dengan bar pembagian unduhan build terbaru per sistem (Windows, Portable,
macOS, Linux, dari `download_count` aset rilis); lalu satu baris kontributor dan push
terakhir. Tingginya mengikuti panel commit di sebelahnya.

## Bahasa

- Bawaan bahasa Inggris: teks di HTML adalah versi Inggris, jadi halaman tetap utuh tanpa
  JavaScript.
- Tombol EN / ID di header. Pilihan disimpan di `localStorage` (`fluxcode-lang`), dan
  `?lang=id` atau `?lang=en` di URL mengalahkannya.
- Elemen statis memakai `data-i18n` (teks), `data-i18n-html` (teks dengan markup milik kita
  sendiri), dan `data-i18n-attr="atribut:kunci"`. Terjemahan Indonesia ada di objek `ID`;
  teks yang ditulis skrip (status, tanggal relatif, tombol unduh, grafik, caption) ada di
  `STR` dan `SHOTS`. Angka dan tanggal diformat dengan `Intl` sesuai bahasa.
- Copy dibuat pendek: satu judul dan satu baris per fitur.

## Data live

Skrip memanggil empat endpoint GitHub: repo, releases (semua halaman), commits, dan
contributors.

- Data terakhir disimpan di `localStorage` (`fluxcode-site-v2`) dan langsung ditampilkan
  saat halaman dibuka berikutnya, lalu diperbarui.
- `fetch` memakai `cache: "no-cache"`, jadi browser memvalidasi ulang dengan ETag; jawaban
  304 dari GitHub tidak memakan batas 60 permintaan per jam.
- Diperbarui tiap 2 menit selama tab terlihat, saat tab kembali dibuka, saat koneksi kembali,
  dan lewat tombol Perbarui sekarang.
- Status: memuat, berhasil (dengan jam pembaruan), batas API habis (dengan jam reset), offline,
  dan gagal. Kalau belum pernah ada data, daftar commit dan kontributor menampilkan pesan
  galat dengan tautan ke GitHub.
- Teks dari API dimasukkan dengan `textContent`, tidak pernah `innerHTML`.

## Responsif dan aksesibilitas

- Di bawah 960 px kolom bertumpuk; di bawah 720 px navigasi atas diganti tab bar bawah dan
  semua tombol selebar layar. Tidak ada scroll horizontal halaman; hanya tab tangkapan layar
  dan tabel perbandingan yang bisa digeser di dalam wadahnya.
- Semua kontrol bisa dicapai dengan keyboard, fokus ditandai garis kuning, Escape menutup
  pemberitahuan unduhan. `prefers-reduced-motion` mematikan semua transisi.
- Tombol dan tautan minimal 40 sampai 44 px tingginya.
