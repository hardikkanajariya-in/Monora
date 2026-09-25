import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type RecordingMode = "combined" | "separate";
type Quality = "balanced" | "high" | "very_high";
type RecordingState =
  | "idle"
  | "starting"
  | "recording"
  | "stopping"
  | "completed"
  | "error";

interface MonitorInfo {
  id: string;
  name: string;
  index: number;
  x: number;
  y: number;
  width: number;
  height: number;
  refresh_rate_hz: number | null;
  is_primary: boolean;
}

interface AudioDeviceInfo {
  id: string;
  name: string;
  is_default: boolean;
}

interface AppSettings {
  selected_monitor_ids: string[];
  recording_mode: RecordingMode;
  fps: number;
  quality: Quality;
  system_audio_enabled: boolean;
  microphone_enabled: boolean;
  selected_microphone_id: string | null;
  output_directory: string;
}

interface RecordingProgress {
  elapsed_secs: number;
  state: RecordingState;
  display_count: number;
  output_paths: string[];
  file_size_bytes: number | null;
  system_audio: boolean;
  microphone: boolean;
}

interface RecordingStatus {
  state: RecordingState;
  progress: RecordingProgress | null;
  last_error: string | null;
}

interface RecordingEntry {
  file_name: string;
  path: string;
  size_bytes: number;
  modified_unix_ms: number;
}

let settings: AppSettings;
let monitors: MonitorInfo[] = [];
let microphones: AudioDeviceInfo[] = [];
let recordings: RecordingEntry[] = [];
let selectedRecordingPath: string | null = null;
let status: RecordingStatus = { state: "idle", progress: null, last_error: null };

const app = document.getElementById("app")!;

function isRecordingActive(): boolean {
  return (
    status.state === "starting" ||
    status.state === "recording" ||
    status.state === "stopping"
  );
}

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatWhen(ms: number): string {
  if (!ms) return "";
  return new Date(ms).toLocaleString();
}

function audioSummary(): string {
  const sys = settings.system_audio_enabled;
  const mic = settings.microphone_enabled;
  if (sys && mic) return "System audio + microphone";
  if (sys) return "System audio";
  if (mic) return "Microphone";
  return "No audio";
}

async function loadRecordings(): Promise<void> {
  recordings = await invoke<RecordingEntry[]>("list_recordings");
  if (
    selectedRecordingPath &&
    !recordings.some((r) => r.path === selectedRecordingPath)
  ) {
    selectedRecordingPath = recordings[0]?.path ?? null;
  } else if (!selectedRecordingPath && recordings.length > 0) {
    selectedRecordingPath = recordings[0].path;
  }
}

async function loadData(): Promise<void> {
  monitors = await invoke<MonitorInfo[]>("get_monitors");
  microphones = await invoke<AudioDeviceInfo[]>("get_audio_devices");
  settings = await invoke<AppSettings>("get_settings");
  status = await invoke<RecordingStatus>("get_recording_status");
  await loadRecordings();
}

async function saveSettings(): Promise<void> {
  await invoke("save_settings", { settings });
}

function toggleMonitor(id: string): void {
  if (isRecordingActive()) return;
  const set = new Set(settings.selected_monitor_ids);
  if (set.has(id)) set.delete(id);
  else set.add(id);
  settings.selected_monitor_ids = Array.from(set);
  saveSettings();
  render();
}

function renderLibraryColumn(): HTMLElement {
  const col = document.createElement("aside");
  col.className = "library-col";

  const header = document.createElement("div");
  header.className = "library-header";
  header.innerHTML = `<h2>Your recordings</h2>`;
  const refreshBtn = document.createElement("button");
  refreshBtn.textContent = "Refresh";
  refreshBtn.disabled = isRecordingActive();
  refreshBtn.onclick = async () => {
    await loadRecordings();
    render();
  };
  header.appendChild(refreshBtn);
  col.appendChild(header);

  const list = document.createElement("div");
  list.className = "library-list";
  if (recordings.length === 0) {
    const empty = document.createElement("div");
    empty.className = "library-empty";
    empty.textContent = "No recordings yet. Finished videos appear here.";
    list.appendChild(empty);
  } else {
    for (const r of recordings) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className =
        "recording-item" + (selectedRecordingPath === r.path ? " active" : "");
      btn.innerHTML = `
        <div class="recording-item-title">${r.file_name}</div>
        <div class="recording-item-meta">${formatBytes(r.size_bytes)} · ${formatWhen(r.modified_unix_ms)}</div>
      `;
      btn.onclick = () => {
        selectedRecordingPath = r.path;
        render();
      };
      list.appendChild(btn);
    }
  }
  col.appendChild(list);

  const playerWrap = document.createElement("div");
  playerWrap.className = "video-player-wrap";
  const label = document.createElement("p");
  label.className = "video-label";
  const video = document.createElement("video");
  video.controls = true;
  video.preload = "metadata";

  if (selectedRecordingPath) {
    const entry = recordings.find((r) => r.path === selectedRecordingPath);
    label.textContent = entry?.file_name ?? selectedRecordingPath;
    video.src = convertFileSrc(selectedRecordingPath);
  } else {
    label.textContent = "Select a recording to play";
  }

  playerWrap.append(label, video);
  col.appendChild(playerWrap);
  return col;
}

function render(): void {
  const recording = status.state === "recording";
  const busy = isRecordingActive();
  const err = status.last_error;
  const progress = status.progress;

  app.innerHTML = "";
  const shell = document.createElement("div");
  shell.className = "app-shell";

  const controls = document.createElement("div");
  controls.className = "controls-col";

  if (err && status.state === "error") {
    const banner = document.createElement("div");
    banner.className = "error-banner";
    banner.textContent = err;
    controls.appendChild(banner);
  }

  const header = document.createElement("header");
  header.className = "app-header";
  const logo = document.createElement("img");
  logo.className = "app-logo";
  logo.src = "/monora-icon.svg";
  logo.width = 28;
  logo.height = 28;
  logo.alt = "";
  const title = document.createElement("h1");
  title.textContent = "Monora";
  const subtitle = document.createElement("p");
  subtitle.className = "app-subtitle";
  subtitle.textContent = "Screen recording for Windows";
  header.append(logo, title, subtitle);
  controls.appendChild(header);

  if (recording || status.state === "starting") {
    const panel = document.createElement("div");
    panel.className = "recording-panel";
    panel.innerHTML = `
      <div class="recording-dot">Recording</div>
      <div class="recording-time">${formatDuration(progress?.elapsed_secs ?? 0)}</div>
      <div class="recording-meta">
        ${progress?.display_count ?? settings.selected_monitor_ids.length} display${(progress?.display_count ?? 1) !== 1 ? "s" : ""}<br/>
        ${audioSummary()}
      </div>
    `;
    controls.appendChild(panel);
    const stopBtn = document.createElement("button");
    stopBtn.className = "primary stop";
    stopBtn.textContent = "Stop recording";
    stopBtn.disabled = status.state === "starting" || status.state === "stopping";
    stopBtn.onclick = () => void invoke("stop_recording");
    controls.appendChild(stopBtn);
  } else if (status.state === "stopping") {
    const panel = document.createElement("div");
    panel.className = "recording-panel";
    panel.innerHTML = `<div class="recording-meta">Finalizing recording…</div>`;
    controls.appendChild(panel);
  } else {
    const scroll = document.createElement("div");
    scroll.className = "controls-scroll";

    const displays = document.createElement("section");
    displays.innerHTML = "<h2>Displays</h2>";
    const list = document.createElement("div");
    list.className = "monitor-list";
    for (const m of monitors) {
      const item = document.createElement("label");
      item.className = "monitor-item" + (busy ? " disabled" : "");
      const checked = settings.selected_monitor_ids.includes(m.id);
      const primary = m.is_primary ? " · Primary" : "";
      const hz = m.refresh_rate_hz ? ` · ${m.refresh_rate_hz} Hz` : "";
      item.innerHTML = `
        <input type="checkbox" ${checked ? "checked" : ""} ${busy ? "disabled" : ""} />
        <div>
          <div>${m.name}</div>
          <div class="monitor-meta">${m.width} × ${m.height}${primary}${hz}</div>
        </div>
      `;
      item.querySelector("input")!.addEventListener("change", () => toggleMonitor(m.id));
      list.appendChild(item);
    }
    displays.appendChild(list);
    scroll.appendChild(displays);

    const rec = document.createElement("section");
    rec.innerHTML = "<h2>Recording</h2>";
    const grid = document.createElement("div");
    grid.className = "settings-grid";

    const modeRow = document.createElement("div");
    modeRow.className = "field-row";
    modeRow.style.gridColumn = "1 / -1";
    modeRow.innerHTML = `<label>Mode</label>`;
    const modeGroup = document.createElement("div");
    modeGroup.className = "radio-group";
    for (const [val, label] of [
      ["combined", "Combined"],
      ["separate", "Separate"],
    ] as const) {
      const r = document.createElement("label");
      r.innerHTML = `<input type="radio" name="mode" value="${val}" ${settings.recording_mode === val ? "checked" : ""} ${busy ? "disabled" : ""} /> ${label}`;
      r.querySelector("input")!.addEventListener("change", () => {
        settings.recording_mode = val;
        saveSettings();
      });
      modeGroup.appendChild(r);
    }
    modeRow.appendChild(modeGroup);
    grid.appendChild(modeRow);

    const fpsRow = document.createElement("div");
    fpsRow.className = "field-row";
    fpsRow.innerHTML = `<label>FPS</label>`;
    const fpsSel = document.createElement("select");
    fpsSel.disabled = busy;
    for (const fps of [30, 60]) {
      const o = document.createElement("option");
      o.value = String(fps);
      o.textContent = String(fps);
      o.selected = settings.fps === fps;
      fpsSel.appendChild(o);
    }
    fpsSel.onchange = () => {
      settings.fps = Number(fpsSel.value);
      saveSettings();
    };
    fpsRow.appendChild(fpsSel);
    grid.appendChild(fpsRow);

    const qualRow = document.createElement("div");
    qualRow.className = "field-row";
    qualRow.innerHTML = `<label>Quality</label>`;
    const qualSel = document.createElement("select");
    qualSel.disabled = busy;
    for (const [val, label] of [
      ["balanced", "Balanced"],
      ["high", "High"],
      ["very_high", "Very High"],
    ] as const) {
      const o = document.createElement("option");
      o.value = val;
      o.textContent = label;
      o.selected = settings.quality === val;
      qualSel.appendChild(o);
    }
    qualSel.onchange = () => {
      settings.quality = qualSel.value as Quality;
      saveSettings();
    };
    qualRow.appendChild(qualSel);
    grid.appendChild(qualRow);
    rec.appendChild(grid);
    scroll.appendChild(rec);

    const audio = document.createElement("section");
    audio.innerHTML = "<h2>Audio</h2>";
    const sysRow = document.createElement("label");
    sysRow.className = "checkbox-row";
    sysRow.innerHTML = `<input type="checkbox" ${settings.system_audio_enabled ? "checked" : ""} ${busy ? "disabled" : ""} /> System audio`;
    sysRow.querySelector("input")!.addEventListener("change", (e) => {
      settings.system_audio_enabled = (e.target as HTMLInputElement).checked;
      saveSettings();
    });
    audio.appendChild(sysRow);
    const micRow = document.createElement("label");
    micRow.className = "checkbox-row";
    micRow.innerHTML = `<input type="checkbox" ${settings.microphone_enabled ? "checked" : ""} ${busy ? "disabled" : ""} /> Microphone`;
    micRow.querySelector("input")!.addEventListener("change", (e) => {
      settings.microphone_enabled = (e.target as HTMLInputElement).checked;
      render();
      saveSettings();
    });
    audio.appendChild(micRow);
    const micSel = document.createElement("select");
    micSel.disabled = busy || !settings.microphone_enabled;
    micSel.style.width = "100%";
    micSel.style.marginBottom = "6px";
    for (const d of microphones) {
      const o = document.createElement("option");
      o.value = d.id;
      o.textContent = d.is_default ? `${d.name} (Default)` : d.name;
      micSel.appendChild(o);
    }
    if (settings.selected_microphone_id) micSel.value = settings.selected_microphone_id;
    else if (microphones.find((m) => m.is_default)) {
      micSel.value = microphones.find((m) => m.is_default)!.id;
    }
    micSel.onchange = () => {
      settings.selected_microphone_id = micSel.value;
      saveSettings();
    };
    audio.appendChild(micSel);
    scroll.appendChild(audio);

    const out = document.createElement("section");
    out.innerHTML = "<h2>Output</h2>";
    const outRow = document.createElement("div");
    outRow.className = "output-row";
    const pathEl = document.createElement("div");
    pathEl.className = "output-path";
    pathEl.textContent = settings.output_directory;
    const changeBtn = document.createElement("button");
    changeBtn.textContent = "Change";
    changeBtn.disabled = busy;
    changeBtn.onclick = async () => {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: settings.output_directory,
      });
      if (selected && typeof selected === "string") {
        settings.output_directory = selected;
        await saveSettings();
        await loadRecordings();
        render();
      }
    };
    outRow.append(pathEl, changeBtn);
    out.appendChild(outRow);
    scroll.appendChild(out);

    controls.appendChild(scroll);

    const startBtn = document.createElement("button");
    startBtn.className = "primary";
    startBtn.textContent = "Start recording";
    startBtn.disabled = busy || settings.selected_monitor_ids.length === 0;
    startBtn.onclick = async () => {
      startBtn.disabled = true;
      status = { ...status, state: "starting", last_error: null };
      render();
      try {
        await invoke("start_recording");
      } catch (e) {
        status = { ...status, state: "error", last_error: String(e) };
        render();
      }
    };
    controls.appendChild(startBtn);
  }

  shell.appendChild(controls);
  shell.appendChild(renderLibraryColumn());
  app.appendChild(shell);
}

async function init(): Promise<void> {
  await loadData();
  render();

  await listen("recording_started", async () => {
    status = await invoke<RecordingStatus>("get_recording_status");
    render();
  });

  await listen("recording_progress", (event) => {
    const p = event.payload as RecordingProgress;
    status = { ...status, state: p.state, progress: p, last_error: null };
    const timeEl = document.querySelector(".recording-time");
    if (timeEl && (p.state === "recording" || p.state === "starting")) {
      timeEl.textContent = formatDuration(p.elapsed_secs);
      return;
    }
    render();
  });

  await listen("recording_stopped", async () => {
    status = await invoke<RecordingStatus>("get_recording_status");
    await loadRecordings();
    render();
  });

  await listen("recording_error", async (event) => {
    const msg = event.payload as string;
    status = { ...status, state: "error", last_error: msg };
    await loadRecordings();
    render();
  });

  await listen("monitor_changed", async () => {
    monitors = await invoke<MonitorInfo[]>("get_monitors");
    if (!isRecordingActive()) render();
  });
}

init().catch((e) => {
  app.innerHTML = `<div class="error-banner">${String(e)}</div>`;
});
