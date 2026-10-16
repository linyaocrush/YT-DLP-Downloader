// Shared settings store and UI shell state (active tab + theme).
//
// Persistence is serialized through a promise queue so rapid keystrokes cannot
// interleave two `settings.json` writes, and responses never overwrite fresher
// local edits.

import * as api from "./api";
import { defaultSettings, type AppSettings, type AppTheme } from "./types";

class SettingsStore {
  value = $state<AppSettings>(defaultSettings());

  private queue: Promise<void> = Promise.resolve();
  private listeners = new Set<() => void>();

  async load(): Promise<void> {
    this.value = await api.getSettings();
    this.notifyChange();
  }

  /** Registers a change listener; returns an unsubscribe function. */
  onChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /**
   * Updates several fields locally and persists them. Returns a promise that
   * resolves once the write has been sent, so callers can read the settings back
   * from Rust afterwards (mirroring the synchronous save + refresh in WPF).
   */
  patch(patch: Partial<AppSettings>): Promise<void> {
    this.setLocalPatch(patch);
    return this.enqueue(patch);
  }

  /**
   * Queues a persist without touching local state. Used when the displayed text
   * should stay exactly as typed while a trimmed value is written to disk.
   */
  persist(patch: Partial<AppSettings>): Promise<void> {
    return this.enqueue(patch);
  }

  /** Updates one field locally and persists it. */
  set<K extends keyof AppSettings>(key: K, value: AppSettings[K]): Promise<void> {
    return this.patch({ [key]: value } as unknown as Partial<AppSettings>);
  }

  /** Updates fields locally without persisting (used when Rust already saved). */
  setLocal<K extends keyof AppSettings>(key: K, value: AppSettings[K]): void {
    this.setLocalPatch({ [key]: value } as unknown as Partial<AppSettings>);
  }

  private setLocalPatch(patch: Partial<AppSettings>): void {
    const target = this.value as unknown as Record<string, unknown>;
    for (const [key, value] of Object.entries(patch)) target[key] = value;
  }

  private enqueue(patch: Partial<AppSettings>): Promise<void> {
    const run = this.queue
      .then(() => api.updateSettings(patch))
      .then(() => undefined)
      .catch(() => undefined)
      .then(() => this.notifyChange());
    this.queue = run;
    return run;
  }

  private notifyChange(): void {
    for (const listener of this.listeners) listener();
  }
}

export const settingsStore = new SettingsStore();

class UiStore {
  tab = $state<"download" | "settings">("download");
  isDark = $state(false);

  private mediaQuery: MediaQueryList | null = null;
  private lastTheme: AppTheme | null = null;

  /** Wires the OS colour-scheme listener and applies the stored theme. */
  initTheme(): void {
    this.mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    this.mediaQuery.addEventListener("change", () => {
      if (settingsStore.value.Theme === "System") this.applyTheme("System");
    });
    this.applyTheme(settingsStore.value.Theme);
  }

  applyTheme(theme: AppTheme): void {
    this.lastTheme = theme;
    const dark = theme === "Dark" || (theme === "System" && !!this.mediaQuery?.matches);
    this.isDark = dark;
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    void api.setWindowTheme(theme);
  }

  /** Re-applies the theme when a settings change altered it (the WPF `Changed` hook). */
  syncTheme(): void {
    if (settingsStore.value.Theme !== this.lastTheme) {
      this.applyTheme(settingsStore.value.Theme);
    }
  }
}

export const ui = new UiStore();