(function () {
  "use strict";

  var THEME_KEY = "rongyok-desk-theme";
  var root = document.documentElement;
  var themeToggle = document.getElementById("theme-toggle");
  var navToggle = document.getElementById("nav-toggle");
  var mobileNav = document.getElementById("mobile-nav");
  var tabs = Array.prototype.slice.call(document.querySelectorAll(".os-tab"));
  var panels = Array.prototype.slice.call(document.querySelectorAll(".os-panel"));
  var copyButtons = Array.prototype.slice.call(document.querySelectorAll(".copy-btn"));

  function applyTheme(theme) {
    var next = theme === "light" ? "light" : "dark";
    root.setAttribute("data-theme", next);
    if (themeToggle) {
      var isLight = next === "light";
      themeToggle.setAttribute("aria-pressed", String(isLight));
      themeToggle.setAttribute(
        "aria-label",
        isLight ? "Switch to dark theme" : "Switch to light theme"
      );
    }
  }

  function saveTheme(theme) {
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch (_err) {
      /* Ignore storage failures in restricted browsing contexts */
    }
  }

  if (themeToggle) {
    applyTheme(root.getAttribute("data-theme") || "dark");
    themeToggle.addEventListener("click", function () {
      var current = root.getAttribute("data-theme") === "light" ? "light" : "dark";
      var next = current === "light" ? "dark" : "light";
      applyTheme(next);
      saveTheme(next);
    });
  }

  if (navToggle && mobileNav) {
    navToggle.addEventListener("click", function () {
      var expanded = navToggle.getAttribute("aria-expanded") === "true";
      navToggle.setAttribute("aria-expanded", String(!expanded));
      mobileNav.hidden = expanded;
    });

    mobileNav.addEventListener("click", function (event) {
      if (event.target && event.target.tagName === "A") {
        navToggle.setAttribute("aria-expanded", "false");
        mobileNav.hidden = true;
      }
    });
  }

  function activateOsTab(osName) {
    tabs.forEach(function (tab) {
      var active = tab.getAttribute("data-os") === osName;
      tab.classList.toggle("is-active", active);
      tab.setAttribute("aria-selected", String(active));
      tab.tabIndex = active ? 0 : -1;
    });

    panels.forEach(function (panel) {
      var active = panel.id === "panel-" + osName;
      panel.classList.toggle("is-active", active);
      panel.hidden = !active;
    });
  }

  function detectDefaultOs() {
    var ua = (navigator.userAgent || "").toLowerCase();
    var platform = (navigator.platform || "").toLowerCase();
    if (ua.indexOf("mac") !== -1 || platform.indexOf("mac") !== -1) {
      return "macos";
    }
    if (
      (ua.indexOf("linux") !== -1 || platform.indexOf("linux") !== -1) &&
      ua.indexOf("android") === -1
    ) {
      return "linux";
    }
    return "windows";
  }

  if (tabs.length > 0 && panels.length > 0) {
    activateOsTab(detectDefaultOs());

    tabs.forEach(function (tab, index) {
      tab.addEventListener("click", function () {
        activateOsTab(tab.getAttribute("data-os"));
      });

      tab.addEventListener("keydown", function (event) {
        var nextIndex = null;
        if (event.key === "ArrowRight") {
          nextIndex = (index + 1) % tabs.length;
        } else if (event.key === "ArrowLeft") {
          nextIndex = (index - 1 + tabs.length) % tabs.length;
        }
        if (nextIndex !== null) {
          event.preventDefault();
          var target = tabs[nextIndex];
          activateOsTab(target.getAttribute("data-os"));
          target.focus();
        }
      });
    });
  }

  function fallbackCopy(text) {
    var textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand("copy");
    } finally {
      document.body.removeChild(textarea);
    }
  }

  copyButtons.forEach(function (btn) {
    btn.addEventListener("click", function () {
      var text = btn.getAttribute("data-copy") || "";
      var label = btn.querySelector(".copy-label");
      var markCopied = function () {
        btn.classList.add("is-copied");
        if (label) {
          label.textContent = "Copied";
        }
        window.setTimeout(function () {
          btn.classList.remove("is-copied");
          if (label) {
            label.textContent = "Copy";
          }
        }, 1600);
      };

      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(markCopied, function () {
          fallbackCopy(text);
          markCopied();
        });
      } else {
        fallbackCopy(text);
        markCopied();
      }
    });
  });
})();
