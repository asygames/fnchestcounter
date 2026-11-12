# FN Chest Counter

Aplicație pentru Windows care numără automat cuferele pe care le deschizi în Fortnite și le păstrează salvate: pe sesiune, pe zi și în total, cu statistici și un overlay mic peste joc.

## Cum recunoaște un cufăr

Aplicația **doar privește ecranul și ascultă sunetul**, la fel ca OBS sau Discord. Nu injectează nimic în joc, nu citește memoria lui și nu trimite taste.

Folosește două semnale, pe care le poți combina:

| Semnal | Ce face |
| --- | --- |
| **Imagine** | Capturează (Windows Graphics Capture) doar zona din jurul promptului de interacțiune, micșorată la 192 px. Se uită după promptul cufărului, învățat la calibrare. Numără când promptul dispare imediat după ce ai ținut apăsată tasta de interacțiune (E sau butonul X de pe controller). |
| **Sunet** | Ascultă ieșirea audio (WASAPI loopback) și compară spectrul cu sunetul de deschidere, învățat la calibrare. Muzica și zgomotul de fundal sunt scăzute automat. |

Moduri: **Oricare** (implicit; dublurile din aceeași deschidere se ignoră), **Amândouă** (cel mai sigur), **Doar imagine**, **Doar sunet**.

Analiza unui cadru durează sub 1 ms, la 15 cadre/s. Aplicația copiază din GPU doar zona promptului, nu tot ecranul.

## Calibrare (o singură dată, ~1 minut)

1. **Zona:** în pagina *Calibrare*, apasă „Captură din joc” și încadrează promptul de lângă cufăr.
2. **Imagine:** stai lângă un cufăr închis până apare promptul și apasă **F8**. Repetă la 3 cufere din locuri diferite.
3. **Sunet:** deschide un cufăr și apasă **F7** imediat după. Repetă încă o dată.
4. **Test:** deschide câteva cufere. Fiecare trebuie numărat o singură dată.

Corecții manuale oricând: **F9** (+1) și **F10** (−1). Toate scurtăturile se pot schimba din *Setări*.

> Overlay-ul se vede peste joc doar în modul **Windowed Fullscreen** (sau în fereastră).

## Instalare și actualizări

Descarcă `FN.Chest.Counter_x.y.z_x64-setup.exe` din [Releases](https://github.com/asygames/fnchestcounter/releases/latest) și rulează-l. Nu cere drepturi de administrator.

Aplicația se **actualizează singură**. La pornire, și apoi o dată la 6 ore, verifică `latest.json` din ultimul release. Dacă găsește o versiune nouă, arată un banner cu „Actualizează acum”. Update-urile sunt semnate, iar aplicația refuză orice fișier nesemnat cu cheia proiectului. Datele, setările și calibrarea rămân neschimbate.

### Cum public o versiune nouă

1. Crește versiunea în `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` și `package.json` (de exemplu `0.1.1`).
2. Fă commit, apoi `git tag v0.1.1 && git push origin main v0.1.1`.
3. GitHub Actions construiește installer-ul semnat și publică release-ul împreună cu `latest.json`. Aplicațiile instalate îl preiau singure.

Semnarea folosește secretele `TAURI_SIGNING_PRIVATE_KEY` și `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` din repo. Cheia privată nu e în repo. **Dacă se pierde, aplicațiile deja instalate nu mai pot primi update-uri.**

Datele se salvează în `%APPDATA%\com.asygames.fnchestcounter\`:

- `data.json`: total, zile, sesiuni, ultimele evenimente;
- `settings.json`: setările;
- `calibration.json`: ce a învățat detecția.

## Dezvoltare

Ai nevoie de Node 22+, Rust stable și, pe Windows, de WebView2 (vine cu Windows 11).

```bash
npm install
npm run tauri dev      # aplicația cu reîncărcare live
npm run tauri build    # installer NSIS în src-tauri/target/release/bundle/nsis
npm run check          # verificare TypeScript/Svelte
cd src-tauri && cargo test --lib   # testele nucleului de detecție
```

`npm run dev` deschide doar interfața în browser, cu date demo: `#stats`, `#calibrate`, `#settings` și `#pulse` pornesc direct pe pagina respectivă.

### Structură

```
src/                  interfața (Svelte 5)
  pages/              Acasă, Statistici, Calibrare, Setări
  lib/                cufărul animat, numărătorul, particulele, graficul, legătura cu Rust
  Overlay.svelte      overlay-ul de peste joc
src-tauri/src/
  detect/visual.rs    șablonul promptului (mască din probe) + potrivire NCC
  detect/audio.rs     amprenta spectrală a sunetului + detecție
  detect/fusion.rs    combinarea semnalelor, fără dubluri
  platform/windows.rs captura, loopback-ul audio, tasta de interacțiune
  engine.rs           starea comună, evenimentele spre interfață
  store.rs            salvarea atomică a datelor
```

Proiect independent; nu e afiliat cu Epic Games.
