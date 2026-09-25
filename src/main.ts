import { invoke } from "@tauri-apps/api/core";
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

let settings: AppSettings;
let monitors: MonitorInfo[] = [];
let microphones: AudioDeviceInfo[] = [];
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

function audioSummary(): string {
  const sys = settings.system_audio_enabled;
  const mic = settings.microphone_enabled;
  if (sys && mic) return "System Audio + Microphone";
  if (sys) return "System Audio";
  if (mic) return "Microphone";
  return "No audio";
}

async function loadData(): Promise<void> {
  monitors = await invoke<MonitorInfo[]>("get_monitors");
  microphones = await invoke<AudioDeviceInfo[]>("get_audio_devices");
  settings = await invoke<AppSettings>("get_settings");
  status = await invoke<RecordingStatus>("get_recording_status");
}

async function saveSettings(): Promise<void> {
  await invoke("save_settings", { settings });
}

function toggleMonitor(id: string): void {
  if (isRecordingActive()) return;
  const set = new Set(settings.selected_monitor_ids);
  if (set.has(id)) {
    set.delete(id);
  } else {
    set.add(id);
  }
  settings.selected_monitor_ids = Array.from(set);
  saveSettings();
  render();
}

function render(): void {
  const recording = status.state === "recording";
  const busy = isRecordingActive();
  const err = status.last_error;
  const progress = status.progress;

  app.innerHTML = "";

  if (err && status.state === "error") {
    const banner = document.createElement("div");
    banner.className = "error-banner";
    banner.textContent = err;
    app.appendChild(banner);
  }

  const title = document.createElement("h1");
  title.textContent = "Simple Recorder";
  app.appendChild(title);

  if (recording || status.state === "starting") {
    const panel = document.createElement("div");
    panel.className = "recording-panel";
    panel.innerHTML = `
      <div class="recording-dot">● Recording</div>
      <div class="recording-time">${formatDuration(progress?.elapsed_secs ?? 0)}</div>
      <div class="recording-meta">
        ${progress?.display_count ?? settings.selected_monitor_ids.length} Display${(progress?.display_count ?? 1) !== 1 ? "s" : ""}<br/>
        ${audioSummary()}
      </div>
    `;
    app.appendChild(panel);

    const stopBtn = document.createElement("button");
    stopBtn.className = "primary stop";
    stopBtn.textContent = "■ STOP";
    stopBtn.disabled = status.state === "starting" || status.state === "stopping";
    stopBtn.onclick = () => void invoke("stop_recording");
    app.appendChild(stopBtn);
    return;
  }

  if (status.state === "stopping") {
    const panel = document.createElement("div");
    panel.className = "recording-panel";
    panel.innerHTML = `<div class="recording-meta">Finalizing recording…</div>`;
    app.appendChild(panel);
    return;
  }

  const displays = document.createElement("section");
  displays.innerHTML = "<h2>Displays</h2>";
  const list = document.createElement("div");
  list.className = "monitor-list";
  for (const m of monitors) {
    const item = document.createElement("label");
    item.className = "monitor-item" + (busy ? " disabled" : "");
    const checked = settings.selected_monitor_ids.includes(m.id);
    const primary = m.is_primary ? " • Primary" : "";
    const hz = m.refresh_rate_hz ? ` • ${m.refresh_rate_hz} Hz` : "";
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
  app.appendChild(displays);

  const rec = document.createElement("section");
  rec.innerHTML = "<h2>Recording</h2>";

  const modeRow = document.createElement("div");
  modeRow.className = "field-row";
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
  rec.appendChild(modeRow);

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
  rec.appendChild(fpsRow);

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
  rec.appendChild(qualRow);
  app.appendChild(rec);

  const audio = document.createElement("section");
  audio.innerHTML = "<h2>Audio</h2>";

  const sysRow = document.createElement("label");
  sysRow.className = "checkbox-row";
  sysRow.innerHTML = `<input type="checkbox" ${settings.system_audio_enabled ? "checked" : ""} ${busy ? "disabled" : ""} /> System Audio`;
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

  const micSelRow = document.createElement("div");
  micSelRow.className = "field-row";
  const micSel = document.createElement("select");
  micSel.disabled = busy || !settings.microphone_enabled;
  for (const d of microphones) {
    const o = document.createElement("option");
    o.value = d.id;
    o.textContent = d.is_default ? `${d.name} (Default)` : d.name;
    micSel.appendChild(o);
  }
  if (settings.selected_microphone_id) {
    micSel.value = settings.selected_microphone_id;
  } else if (microphones.find((m) => m.is_default)) {
    micSel.value = microphones.find((m) => m.is_default)!.id;
  }
  micSel.onchange = () => {
    settings.selected_microphone_id = micSel.value;
    saveSettings();
  };
  micSelRow.appendChild(micSel);
  audio.appendChild(micSelRow);
  app.appendChild(audio);

  const out = document.createElement("section");
  out.innerHTML = "<h2>Output</h2>";
  const pathEl = document.createElement("div");
  pathEl.className = "output-path";
  pathEl.textContent = settings.output_directory;
  out.appendChild(pathEl);
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
      render();
    }
  };
  out.appendChild(changeBtn);
  app.appendChild(out);

  const startBtn = document.createElement("button");
  startBtn.className = "primary";
  startBtn.textContent = "● START RECORDING";
  startBtn.disabled = busy || settings.selected_monitor_ids.length === 0;
  startBtn.onclick = async () => {
    try {
      await invoke("start_recording");
    } catch (e) {
      status = { ...status, state: "error", last_error: String(e) };
      render();
    }
  };
  app.appendChild(startBtn);
}

async function init(): Promise<void> {
  await loadData();
  render();

  await listen("recording_started", () => {
    void invoke<RecordingStatus>("get_recording_status").then((s) => {
      status = s;
      render();
    });
  });

  await listen("recording_progress", (event) => {
    const p = event.payload as RecordingProgress;
    status = {
      ...status,
      state: p.state,
      progress: p,
      last_error: null,
    };
    render();
  });

  await listen("recording_stopped", () => {
    void invoke<RecordingStatus>("get_recording_status").then((s) => {
      status = s;
      render();
    });
  });

  await listen("recording_error", (event) => {
    const msg = event.payload as string;
    status = { ...status, state: "error", last_error: msg };
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
