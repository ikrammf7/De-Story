<p align="center">
  <strong>Desktop AI Video Studio</strong><br />
  Ubah ide cerita menjadi video YouTube & YouTube Shorts lengkap: naskah otomatis, visual AI, suara narator, subtitle karaoke dinamis, dan editor timeline dalam satu aplikasi desktop super ringan.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.1.0-blue.svg?style=flat-square" alt="Version" />
  <img src="https://img.shields.io/badge/Tauri-v2-FFC131?style=flat-square&logo=tauri&logoColor=black" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/Rust-2021-DEA584?style=flat-square&logo=rust&logoColor=black" alt="Rust" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=black" alt="React 19" />
  <img src="https://img.shields.io/badge/TailwindCSS-v4-38B2AC?style=flat-square&logo=tailwind-css&logoColor=white" alt="Tailwind CSS v4" />
  <img src="https://img.shields.io/badge/Platform-Windows-lightgrey?style=flat-square" alt="Platform" />
</p>

---

## 📖 Daftar Isi

1. [Tentang De Story](#-tentang-de-story)
2. [Fitur Unggulan](#-fitur-unggulan)
3. [Arsitektur & Kinerja](#-arsitektur--kinerja)
4. [Prasyarat Sistem](#-prasyarat-sistem)
5. [Instalasi & Menjalankan](#-instalasi--menjalankan)
6. [Panduan Alur Penggunaan (Tutorial)](#-panduan-alur-penggunaan-tutorial)
7. [Integrasi Google Flow & Flow2API](#-integrasi-google-flow--flow2api)
8. [Keamanan & Privasi Data](#-keamanan--privasi-data)
9. [Struktur Direktori Proyek](#-struktur-direktori-proyek)
10. [Perintah & Skrip Pengembangan](#-perintah--skrip-pengembangan)

---

## 🌟 Tentang De Story

**De Story** adalah studio pembuatan konten video berbasis kecerdasan buatan (AI) yang berjalan langsung di desktop. De Story mengotomatisasi seluruh siklus produksi video mulai dari merumuskan premis cerita, memecah adegan naskah (storyboard), menghasilkan gambar atau video animasi dengan konsistensi karakter, memproduksi suara narasi manusiawi berkecepatan tinggi, hingga menyelaraskan teks subtitle karaoke kata-demi-kata (*word-level alignment*) secara lokal menggunakan Whisper CPP.

Hasil akhirnya dapat ditinjau langsung di editor multi-track internal dan diekspor ke video MP4 berkualitas tinggi via FFmpeg terintegrasi.

---

## 🚀 Fitur Unggulan

### 1. 💡 Penulis Naskah & Ide Cerita Cerdas (AI Scriptwriter)
- Cukup masukkan satu kalimat premis atau sinopsis singkat.
- Pilihan durasi fleksibel: **30 detik, 60 detik (Shorts), 3 menit, hingga 5 menit**.
- Format rasio aspek instan: **16:9 (YouTube Landscape)** atau **9:16 (Shorts/TikTok/Reels)**.
- Mendukung berbagai provider LLM: **Google Gemini**, **OpenRouter**, **Groq**, atau **Custom OpenAI-Compatible Endpoint**.
- Pemecahan otomatis menjadi segmen adegan lengkap dengan deskripsi visual, arahan aksi kamera, dan teks narasi.

### 2. 🎨 Generator Visual AI & Lembar Karakter (Storyboard)
- **12+ Preset Gaya Visual Terintegrasi:** Stickman, Anime, 3D Pixar Animation, Archival/Vintage, Comic Book, Claymation, Pixel Art, Watercolor, dll.
- **Konsistensi Karakter (Character Sheet):** AI menghasilkan referensi visual karakter utama agar wajah dan pakaian tetap konsisten di setiap adegan.
- **Dukungan Generator Ganda:**
  - **Higgsfield AI** untuk visual sinematik tinggi.
  - **Google Flow (Flow2API)** untuk generasi gambar Nano Banana dan video Veo 3.1.

### 3. 🎙️ Narasi Suara Alami (Human-like TTS)
- **Google Gemini TTS:** Akses ke 30+ profil suara ekspresif dengan dukungan rotasi pool multi-key otomatis untuk menghindari kuota rate-limit.
- **ElevenLabs TTS:** Kualitas suara narasi premium industri dengan beragam pilihan timbre dan emosi.

### 4. ⚡ Penyelarasan Subtitle Karaoke Offline (Whisper CPP)
- Menggunakan mesin C++ native `whisper.cpp` (model Small) yang berjalan offline di komputer Anda.
- Algoritma dynamic programming **Needleman-Wunsch** menyelaraskan audio dan naskah asli hingga presisi tingkat milidetik per kata.
- Menghasilkan subtitle animasi bergaya karaoke dinamis format ASS (*Advanced SubStation Alpha*) dengan beragam pilihan font kustom.

### 5. 🎬 Editor Timeline Multi-Track & Scrubbing Real-time
- Editor video multi-track responsif: trek visual, trek sulih suara (VO), trek musik latar (BGM), dan trek subtitle.
- Streaming media internal berbasis protokol kustom `studio://` dengan dukungan **HTTP 206 Partial Content (Range Requests)** sehingga scrubbing maju-mundur terasa instan tanpa buffer.
- Efek gerakan kamera dinamis (*Ken Burns: Pan & Zoom*).

### 6. 🛠️ Rendering & Ekspor Video Cepat
- Ekspor video MP4 terkompresi dengan mesin native FFmpeg multithreaded.
- Subtitle karaoke langsung di-burn-in secara presisi ke dalam video akhir.

---

## ⚡ Arsitektur & Kinerja

De Story dibangun menggunakan fondasi **Tauri v2** dengan backend **Rust**, menggantikan arsitektur Electron tradisional:

| Parameter | Versi Lama (Electron) | De Story v1.1.0 (Tauri v2 + Rust) |
| :--- | :--- | :--- |
| **Konsumsi RAM Idle** | ~320 MB – 450 MB | **~40 MB – 60 MB** |
| **Ukuran Distribusi** | ~150 MB+ installer | **Paket biner native super ringkas** |
| **Akses Database** | Node.js bridge | **Native SQLite (`rusqlite`) mode WAL** |
| **Streaming Media** | File buffer lokal berat | **Streaming HTTP 206 Range (`studio://`)** |
| **Keamanan Kunci** | File teks / unencrypted | **Hardware-bound DPAPI OS Native** |

---

## 💻 Prasyarat Sistem

Sebelum menjalankan atau membangun De Story dari sumber kode, pastikan sistem Anda memenuhi persyaratan berikut:

- **Sistem Operasi:** Windows 10/11 (64-bit), macOS 12+ (Apple Silicon / Intel), atau Linux (x64)
- **Node.js:** Versi LTS (v18.0.0 atau lebih baru) & `npm`
- **Rust Toolchain:** Rust & Cargo versi 1.78.0 atau lebih baru ([instal via rustup](https://rustup.rs))
- **Build Tools (Khusus Windows):** *Visual Studio C++ Build Tools* (termasuk Windows SDK) untuk kompilasi Rust & Whisper CPP.

---

## 📦 Instalasi & Menjalankan

### 1. Kloning Repositori
```bash
git clone https://github.com/username/de-story.git
cd de-story
```

### 2. Pasang Dependensi Node.js
```bash
npm install
```

### 3. Menjalankan Mode Pengembangan (Development)
Untuk menjalankan aplikasi dalam mode hot-reload:
```bash
npm run tauri:dev
```
> **Shortcut Praktis:**
> - Di Windows: Klik dua kali file `START - WIN.bat`.
> - Di macOS: Klik dua kali file `START - MAC.command`.

### 4. Membangun Paket Installer Rilis (.exe / .msi / .dmg / .deb)

#### A. Build Lokal (Sesuai OS yang Sedang Digunakan)
Tauri v2 dan runtime embedded Flow2API memerlukan pustaka native dari sistem operasi host. Jalankan perintah berikut di komputer masing-masing:
```bash
npm run tauri:build
```
Hasil installer otomatis terbentuk di:
- **Windows:** `src-tauri/target/release/bundle/nsis/` (`.exe`) dan `bundle/msi/` (`.msi`)
- **macOS:** `src-tauri/target/release/bundle/dmg/` (`.dmg`) dan `bundle/macos/` (`.app`)
- **Linux:** `src-tauri/target/release/bundle/deb/` (`.deb`) dan `bundle/appimage/` (`.AppImage`)

#### B. Build Multi-Platform Otomatis (GitHub Actions CI/CD)
Repositori ini telah dilengkapi dengan workflow CI/CD GitHub Actions di [`.github/workflows/release.yml`](.github/workflows/release.yml) yang dapat mengompilasi installer ketiga sistem operasi sekaligus secara otomatis:
1. Push kode ke repositori GitHub Anda.
2. Buat git tag baru (contoh: `git tag v1.1.0 && git push origin v1.1.0`), atau jalankan workflow secara manual melalui tab **Actions** → **Release & Multi-Platform Build** → **Run workflow**.
3. Runner GitHub (Windows, macOS Intel/ARM, dan Ubuntu) akan otomatis membangun semua installer dan mempublikasikannya langsung ke tab **Releases** GitHub Anda.

---

## 📝 Panduan Alur Penggunaan (Tutorial)

```
[Pengaturan API] ➔ [1. Ide Cerita] ➔ [2. Naskah Adegan] ➔ [3. Storyboard & Aset] ➔ [4. Timeline & Ekspor]
```

### Langkah 0: Konfigurasi API Keys
1. Buka aplikasi **De Story**.
2. Masuk ke menu **Pengaturan** (ikon gerigi di sudut kanan atas).
3. Pada tab **Layanan AI dan kunci**, lengkapi kredensial yang dibutuhkan:
   - **Google Gemini API Key:** (Bisa memasukkan beberapa kunci sekaligus dipisahkan koma atau baris baru untuk rotasi otomatis).
   - **ElevenLabs API Key:** (Opsional, jika ingin narator ElevenLabs).
   - **Higgsfield AI Key / Google Flow:** Untuk generator visual adegan.
4. Klik **Periksa** untuk memastikan status kunci aktif (tercentang hijau).

### Langkah 1: Merumuskan Ide Cerita
1. Di halaman **Beranda**, ketik ide atau sinopsis cerita pada kotak input (atau pilih salah satu saran ide cepat).
2. Tentukan parameter video:
   - **Durasi:** 30s, 60s (Shorts), 3 menit, atau 5 menit.
   - **Bahasa:** Indonesia, Inggris, dsb.
   - **Format:** 16:9 (YouTube) atau 9:16 (Shorts/TikTok).
   - **Gaya Visual:** Pilih gaya ilustrasi favorit (misal: *Stickman*, *Anime*, atau *3D Animation*).
   - **Suara Narator:** Pilih profil suara TTS yang diinginkan.
3. Klik tombol **Mulai**.

### Langkah 2: Menyusun Naskah (Script)
1. AI akan menyusun daftar adegan lengkap beserta deskripsi visual dan naskah suaranya.
2. Anda dapat membaca ulang, menyunting teks dialog/narasi, menambah adegan baru, atau menghapus adegan yang kurang pas.
3. Setelah naskah selesai, klik **Lanjut ke Storyboard**.

### Langkah 3: Produksi Storyboard & Visual
1. Klik **Buat Lembar Karakter** untuk mengunci konsistensi wajah tokoh utama.
2. Klik **Generate Visual Semua** (atau buat per adegan) untuk memproses gambar/video AI.
3. Klik **Generate Suara Semua** untuk memproduksi suara narasi tiap adegan.
4. Whisper CPP lokal akan secara otomatis menyelaraskan suara dengan teks menjadi subtitle presisi.
5. Klik **Buka di Editor**.

### Langkah 4: Editing Timeline & Ekspor
1. Di halaman Editor, Anda dapat memutar pratinjau video secara real-time.
2. Sesuaikan durasi klip, aktifkan efek kamera (*Pan & Zoom*), atur volume suara narator, dan tambahkan musik latar (BGM).
3. Kustomisasi tampilan subtitle karaoke (posisi teks, font, ukuran, dan warna highlight).
4. Klik tombol **Ekspor Video** di pojok kanan atas. De Story akan merender video ke format `.mp4` dan menyimpannya di folder `Videos/De Story/`.

---

## 🧩 Integrasi Google Flow & Flow2API

De Story menyertakan runtime bawaan **Flow2API** (berbasis Python Standalone & Playwright) yang disiapkan secara otomatis tanpa mengharuskan pengguna memasang interpreter Python eksternal.

### Pilihan Solusi CAPTCHA reCAPTCHA Enterprise
Google Flow memerlukan validasi reCAPTCHA Enterprise. Semua metode dapat dikonfigurasi melalui **Panel Admin Flow2API**:

| Metode | Kebutuhan | Karakteristik |
| :--- | :--- | :--- |
| **`browser`** | Headed Chromium internal | Berjalan otomatis di latar belakang menggunakan Playwright bawaan. |
| **`extension`** | Google Chrome + Akun Google | Menggunakan browser Chrome pribadi Anda melalui ekstensi internal. |
| **`yescaptcha` / `capsolver` / `2captcha`** | API Key layanan captcha | Solusi otomatis pihak ketiga berbayar yang stabil. |
| **`flow2api_team`** | Tanpa konfigurasi | Layanan resolver bawaan Flow2API. |

### Panduan Menggunakan Extension Worker (Rekomendasi Hemat)
1. Buka **Pengaturan → Layanan AI dan kunci → Google Flow (Flow2API)**.
2. Di bagian **Extension Worker**, masukkan port tetap loopback (contoh: `8765`), lalu klik **Simpan dan nyalakan ulang**.
3. Salin **WebSocket URL** dan **Kunci** yang ditampilkan.
4. Klik tombol **Buka folder ekstensi**.
5. Buka `chrome://extensions` di Google Chrome Anda, aktifkan **Developer mode**, klik **Load unpacked**, dan pilih folder ekstensi tersebut.
6. Pada opsi ekstensi, tempelkan WebSocket URL & Kunci. Pastikan Anda telah login ke akun Google Labs Anda.
7. Pilih metode `extension` pada panel admin Flow2API.

---

## 🔒 Keamanan & Privasi Data

De Story dirancang dengan prinsip privasi data mutlak (*Privacy by Design*):

1. **Enkripsi Kunci Tingkat Sistem Operasi:**
   - Di Windows, seluruh API Key disimpan terenkripsi menggunakan **Windows DPAPI (`CryptProtectData`)**. Kunci hanya dapat didekripsi oleh pengguna Windows aktif pada perangkat fisik tersebut.
   - Di macOS dan Linux, data dilindungi menggunakan *OS Keyring / Secret Service* dengan selubung autentikasi AES-256-GCM.
2. **Pemisahan Data Pengguna & Repositori:**
   - Semua database proyek (`studio.db`), database cache Flow2API (`flow.db`), aset video, gambar render, dan audio disimpan di folder sistem aplikasi:
     - **Windows:** `%APPDATA%\De Story\` (`C:\Users\<User>\AppData\Roaming\De Story\`)
     - **macOS:** `~/Library/Application Support/De Story/`
     - **Linux:** `~/.config/De Story/`
   - Data ini **sepenuhnya berada di luar folder repositori**, sehingga aman dari risiko kebocoran saat Anda melakukan commit atau push ke GitHub.
3. **Aman untuk Build & Distribusi:**
   - Saat Anda menjalankan `npm run tauri:build`, biner installer yang dihasilkan murni hanya berisi kode aplikasi tanpa menyertakan API Key, riwayat proyek, atau kredensial pribadi Anda.

---

## 📁 Struktur Direktori Proyek

```
de-story/
├── src-tauri/                     # Backend Utama (Rust & Tauri v2)
│   ├── src/
│   │   ├── commands/              # Handler IPC Commands (Proyek, Ekspor, Settings)
│   │   ├── db/                    # SQLite engine (rusqlite) & migrasi schema
│   │   ├── media/                 # FFmpeg video exporter & engine subtitle ASS
│   │   ├── services/              # Integrasi AI (Gemini, ElevenLabs, Higgsfield, LLM)
│   │   ├── speech/                # Whisper CPP engine & Needleman-Wunsch alignment
│   │   ├── paths.rs               # Manajemen folder data pengguna (%APPDATA%\De Story)
│   │   ├── protocol.rs            # Custom protocol studio:// (HTTP 206 Range Streaming)
│   │   ├── secrets.rs             # Windows DPAPI native encryption (CryptProtectData)
│   │   └── main.rs                # Entrypoint biner aplikasi
│   ├── Cargo.toml                 # Konfigurasi dependensi Rust backend
│   └── tauri.conf.json            # Konfigurasi bundle, window, & permissions Tauri v2
│
├── src/
│   ├── renderer/                  # Frontend UI (React 19 + Tailwind v4 + Zustand)
│   │   ├── src/
│   │   │   ├── components/        # Komponen UI modular (Timeline, Audio, Player, Logo)
│   │   │   ├── pages/             # Tampilan utama (Home, Project, Script, Editor, Settings)
│   │   │   └── store/             # Global state management (Zustand)
│   │   └── index.html             # HTML shell dengan Content Security Policy ketat
│   └── shared/                    # Tipe data TypeScript, model AI, dan katalog visual
│
├── resources/                     # Aset biner & statis (Model Whisper, font TrueType)
├── scripts/                       # Skrip otomatisasi build & staging Flow2API
├── package.json                   # Konfigurasi npm & skrip runner
└── README.md                      # Dokumentasi proyek
```

---

## 🔧 Perintah & Skrip Pengembangan

| Perintah | Deskripsi |
| :--- | :--- |
| `npm run tauri:dev` | Menjalankan aplikasi dalam mode pengembangan dengan Vite hot reload. |
| `npm run tauri:build` | Membangun biner installer rilis untuk sistem operasi saat ini. |
| `npm run typecheck` | Memvalidasi seluruh tipe TypeScript (renderer & shared) tanpa output file. |
| `npm run flow2api:stage` | Mengunduh dan menata runtime Python & Chromium Playwright untuk Flow2API. |
| `npm run flow2api:smoke` | Menguji kesiapan integrasi server Flow2API dan browser Playwright. |

---

## 📄 Lisensi & Kredit

- **Modifikasi & Pengembangan:** [ikram.mf7](https://github.com/ikrammf7)
- **Proyek Original:** [Bang Story](https://github.com/bangtutorial/bang-story) oleh **Bang Tutorial**
- Didukung oleh komunitas open-source luar biasa: [Tauri](https://tauri.app/), [Rust](https://www.rust-lang.org/), [React](https://react.dev/), [whisper.cpp](https://github.com/ggerganov/whisper.cpp), dan [FFmpeg](https://ffmpeg.org/).
