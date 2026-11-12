// Actualizări automate: verifică release-urile de pe GitHub (latest.json semnat),
// descarcă, instalează și repornește aplicația. Datele rămân în %APPDATA%.

import { isTauri } from "./api";
import type { Update } from "@tauri-apps/plugin-updater";

type Status = "idle" | "checking" | "none" | "available" | "downloading" | "installing" | "error";

class Updater {
  status = $state<Status>("idle");
  version = $state("");
  notes = $state("");
  progress = $state(0);
  error = $state("");
  dismissed = $state(false);
  private update: Update | null = null;
  private timer: ReturnType<typeof setInterval> | undefined;

  start() {
    if (!isTauri || this.timer) return;
    setTimeout(() => this.check(true), 4000);
    this.timer = setInterval(() => this.check(true), 6 * 3600 * 1000);
  }

  async check(silent = false) {
    if (!isTauri) {
      this.status = "none";
      return;
    }
    if (this.status === "downloading" || this.status === "installing") return;
    this.status = "checking";
    this.error = "";
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const u = await check();
      if (u) {
        this.update = u;
        this.version = u.version;
        this.notes = (u.body ?? "").trim();
        this.status = "available";
        this.dismissed = false;
      } else {
        this.status = "none";
      }
    } catch (e) {
      // Fără internet sau fără release încă: nu deranjăm utilizatorul la verificarea automată.
      this.status = silent ? "idle" : "error";
      this.error = String(e);
    }
  }

  async install() {
    if (!this.update) return;
    this.status = "downloading";
    this.progress = 0;
    let total = 0;
    let got = 0;
    try {
      await this.update.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          got += ev.data.chunkLength;
          this.progress = total ? got / total : 0;
        } else if (ev.event === "Finished") {
          this.status = "installing";
          this.progress = 1;
        }
      });
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      this.status = "error";
      this.error = String(e);
    }
  }
}

export const updater = new Updater();
