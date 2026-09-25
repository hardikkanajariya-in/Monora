(function () {
  const TASKBAR_H = 40;
  const MIN_W = 320;
  const MIN_H = 180;

  const desktop = document.getElementById("desktop");
  const windowLayer = document.getElementById("window-layer");
  const taskbarWindows = document.getElementById("taskbar-windows");
  const startMenu = document.getElementById("start-menu");
  const startBtn = document.getElementById("start-button");
  const showDesktopBtn = document.getElementById("show-desktop");
  const overlay = document.getElementById("desktop-overlay");

  if (!windowLayer) return;

  let zTop = 20;
  const windows = new Map();

  function workArea() {
    return {
      w: window.innerWidth,
      h: window.innerHeight - TASKBAR_H,
    };
  }

  function clampWindow(el, x, y, w, h) {
    const area = workArea();
    const maxX = Math.max(0, area.w - w);
    const maxY = Math.max(0, area.h - h);
    return {
      x: Math.min(Math.max(0, x), maxX),
      y: Math.min(Math.max(0, y), maxY),
      w: Math.min(w, area.w),
      h: Math.min(h, area.h),
    };
  }

  function applyGeometry(el, geom) {
    el.style.left = `${geom.x}px`;
    el.style.top = `${geom.y}px`;
    el.style.width = `${geom.w}px`;
    if (geom.h != null) el.style.height = `${geom.h}px`;
    else el.style.height = "";
  }

  function focusWindow(id) {
    const state = windows.get(id);
    if (!state || !state.el) return;
    windowLayer.querySelectorAll(".win").forEach((w) => {
      w.classList.toggle("is-focused", w === state.el);
      w.classList.toggle("is-inactive", w !== state.el);
    });
    state.el.style.zIndex = String(++zTop);
    taskbarWindows.querySelectorAll(".taskbar-window-btn").forEach((btn) => {
      btn.classList.toggle("is-active", btn.dataset.windowId === id && !state.minimized);
      btn.classList.toggle("is-flash", false);
    });
    state.focused = true;
  }

  function openWindow(id) {
    const state = windows.get(id);
    if (!state) return;
    state.closed = false;
    state.minimized = false;
    state.el.classList.remove("is-minimized", "is-closed");
    taskbarWindows.querySelector(`[data-window-id="${id}"]`)?.classList.remove("is-flash");
    focusWindow(id);
    updateTaskbarButton(id);
  }

  function minimizeWindow(id) {
    const state = windows.get(id);
    if (!state) return;
    state.minimized = true;
    state.el.classList.add("is-minimized");
    taskbarWindows.querySelectorAll(".taskbar-window-btn").forEach((btn) => {
      btn.classList.toggle("is-active", false);
      if (btn.dataset.windowId === id) btn.classList.add("is-flash");
    });
    closeStartMenu();
  }

  function closeWindow(id) {
    const state = windows.get(id);
    if (!state) return;
    state.closed = true;
    state.minimized = true;
    state.el.classList.add("is-minimized", "is-closed");
    updateTaskbarButton(id);
  }

  function toggleMaximize(id) {
    const state = windows.get(id);
    if (!state || state.minimized || state.closed) return;
    const el = state.el;
    if (el.classList.contains("is-maximized")) {
      el.classList.remove("is-maximized");
      if (state.restore) applyGeometry(el, state.restore);
      el.querySelector('[data-action="maximize"]')?.classList.remove("is-restored");
    } else {
      const rect = el.getBoundingClientRect();
      state.restore = {
        x: rect.left,
        y: rect.top,
        w: rect.width,
        h: rect.height,
      };
      const area = workArea();
      applyGeometry(el, { x: 0, y: 0, w: area.w, h: area.h });
      el.classList.add("is-maximized");
      el.querySelector('[data-action="maximize"]')?.classList.add("is-restored");
    }
  }

  function taskbarToggle(id) {
    const state = windows.get(id);
    if (!state) return;
    if (state.closed) {
      openWindow(id);
      return;
    }
    if (state.minimized) {
      openWindow(id);
      return;
    }
    if (state.el.classList.contains("is-focused")) {
      minimizeWindow(id);
    } else {
      focusWindow(id);
    }
  }

  function updateTaskbarButton(id) {
    const btn = taskbarWindows.querySelector(`[data-window-id="${id}"]`);
    const state = windows.get(id);
    if (!btn || !state) return;
    btn.classList.toggle("is-running", !state.closed);
    btn.classList.toggle("is-parked", state.closed);
    btn.classList.toggle(
      "is-active",
      !state.minimized && !state.closed && state.el.classList.contains("is-focused")
    );
  }

  function createTaskbarButton(state) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "taskbar-window-btn is-running";
    btn.dataset.windowId = state.id;
    const title = state.el.querySelector(".titlebar-text")?.textContent || state.id;
    const icon = state.el.querySelector(".titlebar-icon")?.getAttribute("src") || "assets/monora-icon.svg";
    btn.innerHTML = `<img src="${icon}" width="16" height="16" alt="" /><span>${title}</span>`;
    btn.addEventListener("click", () => taskbarToggle(state.id));
    taskbarWindows.appendChild(btn);
  }

  function initWindow(el) {
    const id = el.dataset.windowId;
    if (!id) return;

    const defaultW = Number(el.dataset.defaultW) || 480;
    const defaultX = Number(el.dataset.defaultX) || 40;
    const defaultY = Number(el.dataset.defaultY) || 40;

    el.style.position = "absolute";
    const geom = clampWindow(el, defaultX, defaultY, defaultW, MIN_H);
    applyGeometry(el, geom);

    const state = {
      id,
      el,
      minimized: false,
      closed: false,
      focused: false,
      restore: null,
    };
    windows.set(id, state);
    createTaskbarButton(state);

    el.addEventListener("mousedown", () => focusWindow(id));

    el.querySelectorAll("[data-action]").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        const action = btn.getAttribute("data-action");
        if (action === "minimize") minimizeWindow(id);
        else if (action === "maximize") toggleMaximize(id);
        else if (action === "close") closeWindow(id);
      });
    });

    const handle = el.querySelector("[data-drag-handle]");
    if (handle) {
      handle.addEventListener("mousedown", (e) => {
        if (e.button !== 0 || el.classList.contains("is-maximized")) return;
        e.preventDefault();
        focusWindow(id);
        const startX = e.clientX;
        const startY = e.clientY;
        const rect = el.getBoundingClientRect();
        const originX = rect.left;
        const originY = rect.top;

        function onMove(ev) {
          const dx = ev.clientX - startX;
          const dy = ev.clientY - startY;
          const g = clampWindow(el, originX + dx, originY + dy, rect.width, rect.height);
          applyGeometry(el, g);
        }
        function onUp() {
          document.removeEventListener("mousemove", onMove);
          document.removeEventListener("mouseup", onUp);
        }
        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
      });
      handle.addEventListener("dblclick", (e) => {
        if (!e.target.closest("[data-action]")) toggleMaximize(id);
      });
    }

    const resizeHandle = el.querySelector("[data-resize-handle]");
    if (resizeHandle) {
      resizeHandle.addEventListener("mousedown", (e) => {
        if (e.button !== 0 || el.classList.contains("is-maximized")) return;
        e.preventDefault();
        e.stopPropagation();
        focusWindow(id);
        const rect = el.getBoundingClientRect();
        const startX = e.clientX;
        const startY = e.clientY;
        const startW = rect.width;
        const startH = rect.height;

        function onMove(ev) {
          const w = Math.max(MIN_W, startW + (ev.clientX - startX));
          const h = Math.max(MIN_H, startH + (ev.clientY - startY));
          const g = clampWindow(el, rect.left, rect.top, w, h);
          applyGeometry(el, g);
        }
        function onUp() {
          document.removeEventListener("mousemove", onMove);
          document.removeEventListener("mouseup", onUp);
        }
        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
      });
    }
  }

  windowLayer.querySelectorAll(".win").forEach(initWindow);
  focusWindow("welcome");

  function closeStartMenu() {
    startMenu?.setAttribute("hidden", "");
    startBtn?.classList.remove("is-pressed");
    startBtn?.setAttribute("aria-expanded", "false");
  }

  function openStartMenu() {
    startMenu?.removeAttribute("hidden");
    startBtn?.classList.add("is-pressed");
    startBtn?.setAttribute("aria-expanded", "true");
  }

  startBtn?.addEventListener("click", (e) => {
    e.stopPropagation();
    if (startMenu?.hasAttribute("hidden")) openStartMenu();
    else closeStartMenu();
  });

  document.addEventListener("click", (e) => {
    if (!e.target.closest("#start-menu") && !e.target.closest("#start-button")) {
      closeStartMenu();
    }
  });

  startMenu?.addEventListener("click", (e) => e.stopPropagation());

  document.querySelectorAll("[data-open]").forEach((node) => {
    node.addEventListener("click", () => {
      const id = node.getAttribute("data-open");
      if (id) openWindow(id);
      closeStartMenu();
    });
  });

  document.getElementById("start-shutdown")?.addEventListener("click", () => {
    closeStartMenu();
    windowLayer.querySelectorAll(".win").forEach((w) => {
      const id = w.dataset.windowId;
      if (id) closeWindow(id);
    });
    overlay?.removeAttribute("hidden");
    setTimeout(() => overlay?.setAttribute("hidden", ""), 400);
  });

  let showDesktopActive = false;
  showDesktopBtn?.addEventListener("mousedown", () => {
    showDesktopActive = true;
    windowLayer.querySelectorAll(".win").forEach((w) => w.classList.add("is-peek-hidden"));
    desktop?.classList.add("is-showing-desktop");
  });
  showDesktopBtn?.addEventListener("mouseup", () => {
    if (!showDesktopActive) return;
    showDesktopActive = false;
    windowLayer.querySelectorAll(".win").forEach((w) => w.classList.remove("is-peek-hidden"));
    desktop?.classList.remove("is-showing-desktop");
  });
  showDesktopBtn?.addEventListener("mouseleave", () => {
    if (showDesktopActive) {
      showDesktopActive = false;
      windowLayer.querySelectorAll(".win").forEach((w) => w.classList.remove("is-peek-hidden"));
      desktop?.classList.remove("is-showing-desktop");
    }
  });

  document.getElementById("desktop-icons")?.addEventListener("click", (e) => {
    const icon = e.target.closest(".desktop-icon");
    document.querySelectorAll(".desktop-icon").forEach((i) => i.classList.remove("is-selected"));
    if (icon) icon.classList.add("is-selected");
  });

  document.getElementById("desktop-icons")?.addEventListener("dblclick", (e) => {
    const icon = e.target.closest(".desktop-icon");
    const id = icon?.getAttribute("data-open");
    if (id) openWindow(id);
  });

  desktop?.addEventListener("click", (e) => {
    if (e.target === desktop || e.target.classList.contains("window-layer")) {
      document.querySelectorAll(".desktop-icon").forEach((i) => i.classList.remove("is-selected"));
    }
  });

  window.addEventListener("resize", () => {
    windows.forEach((state) => {
      const el = state.el;
      if (el.classList.contains("is-maximized")) {
        const area = workArea();
        applyGeometry(el, { x: 0, y: 0, w: area.w, h: area.h });
      } else {
        const rect = el.getBoundingClientRect();
        const g = clampWindow(el, rect.left, rect.top, rect.width, rect.height);
        applyGeometry(el, g);
      }
    });
  });

  function pad(n) {
    return String(n).padStart(2, "0");
  }

  function tickClock() {
    const el = document.getElementById("taskbar-clock");
    if (!el) return;
    const d = new Date();
    const h = d.getHours();
    const m = d.getMinutes();
    const hr = h % 12 || 12;
    el.textContent = `${hr}:${pad(m)} ${h >= 12 ? "PM" : "AM"}`;
  }

  setInterval(tickClock, 1000);
  tickClock();
})();
