(function () {
  const REPO =
    window.MONORA_GITHUB_REPO && !window.MONORA_GITHUB_REPO.includes("YOUR_GITHUB")
      ? window.MONORA_GITHUB_REPO
      : null;

  const $ = (sel) => document.querySelector(sel);

  function pad(n) {
    return String(n).padStart(2, "0");
  }

  function tickClock() {
    const el = $("#taskbar-clock");
    if (!el) return;
    const d = new Date();
    const h = d.getHours();
    const m = d.getMinutes();
    const use12 = true;
    if (use12) {
      const hr = h % 12 || 12;
      el.textContent = `${hr}:${pad(m)} ${h >= 12 ? "PM" : "AM"}`;
    } else {
      el.textContent = `${pad(h)}:${pad(m)}`;
    }
  }

  setInterval(tickClock, 1000);
  tickClock();

  const startBtn = $("#start-button");
  const startMenu = $("#start-menu");
  if (startBtn && startMenu) {
    startBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      startMenu.classList.toggle("open");
    });
    document.addEventListener("click", () => startMenu.classList.remove("open"));
    startMenu.addEventListener("click", (e) => e.stopPropagation());
  }

  document.querySelectorAll("[data-scroll]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const id = btn.getAttribute("data-scroll");
      const target = document.getElementById(id);
      if (target) target.scrollIntoView({ behavior: "smooth", block: "start" });
      startMenu?.classList.remove("open");
    });
  });

  function setDownloadState({ portable, installer, version, releaseUrl }) {
    const portableBtn = $("#download-portable");
    const installerBtn = $("#download-installer");
    const versionEl = $("#release-version");
    const statusEl = $("#download-status");

    if (versionEl && version) versionEl.textContent = version;

    if (portableBtn && portable) {
      portableBtn.href = portable;
      portableBtn.classList.remove("disabled");
      portableBtn.removeAttribute("aria-disabled");
    }
    if (installerBtn && installer) {
      installerBtn.href = installer;
      installerBtn.classList.remove("disabled");
      installerBtn.removeAttribute("aria-disabled");
    }
    if (statusEl) {
      if (releaseUrl) {
        statusEl.innerHTML = `Current release: <a href="${releaseUrl}" target="_blank" rel="noopener">GitHub Releases</a>.`;
      } else if (!REPO) {
        statusEl.textContent =
          "Configure the repository in website/js/repo-config.js to enable direct download links.";
      }
    }
  }

  async function loadLatestRelease() {
    if (!REPO) {
      setDownloadState({});
      return;
    }

    const statusEl = $("#download-status");
    if (statusEl) statusEl.textContent = "Loading release information…";

    try {
      const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      const assets = data.assets || [];

      const portable = assets.find((a) => /portable\.zip$/i.test(a.name))?.browser_download_url;
      const installer = assets.find((a) => /setup\.exe$/i.test(a.name))?.browser_download_url;

      setDownloadState({
        portable,
        installer,
        version: data.tag_name || data.name,
        releaseUrl: data.html_url,
      });

      if (!portable && !installer && statusEl) {
        statusEl.textContent = "No release packages are published yet. See GitHub Releases for updates.";
      }
    } catch {
      if (statusEl) {
        statusEl.textContent =
          "Release information could not be loaded. Download builds from the GitHub Releases page.";
      }
      const releasesLink = $("#releases-fallback");
      if (releasesLink && REPO) {
        releasesLink.href = `https://github.com/${REPO}/releases`;
        releasesLink.hidden = false;
      }
    }
  }

  loadLatestRelease();

  const ghLink = $("#github-link");
  const srcLink = $("#source-link");
  if (REPO && ghLink) ghLink.href = `https://github.com/${REPO}`;
  if (REPO && srcLink) srcLink.href = `https://github.com/${REPO}`;
})();
