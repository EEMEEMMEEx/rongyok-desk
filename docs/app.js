(function () {
  "use strict";

  var THEME_KEY = "rongyok-desk-theme";
  var LANG_KEY = "rongyok-desk-lang";
  var root = document.documentElement;
  var themeToggle = document.getElementById("theme-toggle");
  var langToggle = document.getElementById("lang-toggle");
  var langLabel = document.getElementById("lang-label");
  var navToggle = document.getElementById("nav-toggle");
  var mobileNav = document.getElementById("mobile-nav");
  var tabs = Array.prototype.slice.call(document.querySelectorAll(".os-tab"));
  var panels = Array.prototype.slice.call(document.querySelectorAll(".os-panel"));
  var copyButtons = Array.prototype.slice.call(document.querySelectorAll(".copy-btn"));

  function getCurrentLang() {
    return root.getAttribute("data-lang") === "th" ? "th" : "en";
  }

  function applyTheme(theme) {
    var next = theme === "light" ? "light" : "dark";
    root.setAttribute("data-theme", next);
    if (themeToggle) {
      var isLight = next === "light";
      var isTh = getCurrentLang() === "th";
      themeToggle.setAttribute("aria-pressed", String(isLight));
      themeToggle.setAttribute(
        "aria-label",
        isTh
          ? isLight
            ? "สลับเป็นธีมมืด"
            : "สลับเป็นธีมสว่าง"
          : isLight
            ? "Switch to dark theme"
            : "Switch to light theme"
      );
      themeToggle.setAttribute(
        "title",
        isTh ? "สลับธีมสี" : "Toggle color theme"
      );
    }
  }

  function saveSetting(key, value) {
    try {
      localStorage.setItem(key, value);
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
      saveSetting(THEME_KEY, next);
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
        var isTh = getCurrentLang() === "th";
        btn.classList.add("is-copied");
        if (label) {
          label.textContent = isTh ? "คัดลอกแล้ว" : "Copied";
        }
        window.setTimeout(function () {
          btn.classList.remove("is-copied");
          if (label) {
            label.textContent = getCurrentLang() === "th" ? "คัดลอก" : "Copy";
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

  // Bilingual TH/EN switching derived from live English DOM values (zero innerHTML).
  function createTokenNode(token) {
    if (typeof token === "string") {
      return document.createTextNode(token);
    }
    var node = document.createElement(token.tag);
    if (token.className) {
      node.className = token.className;
    }
    node.textContent = token.text;
    return node;
  }

  function setMixedContent(el, tokens) {
    if (!el) return;
    while (el.firstChild) {
      el.removeChild(el.firstChild);
    }
    tokens.forEach(function (token) {
      el.appendChild(createTokenNode(token));
    });
  }

  function setLeadingMixedContent(el, beforeNode, tokens) {
    if (!el || !beforeNode || beforeNode.parentNode !== el) return;
    while (el.firstChild && el.firstChild !== beforeNode) {
      el.removeChild(el.firstChild);
    }
    tokens.forEach(function (token) {
      el.insertBefore(createTokenNode(token), beforeNode);
    });
  }

  var brandVerEl = document.querySelector(".brand-version");
  var rawVer = brandVerEl ? brandVerEl.textContent.trim() : "v1.2.0";
  var cleanVer = rawVer.replace(/^v/i, "") || "1.2.0";
  var platformCodes = document.querySelectorAll("#downloads .platform-card .asset-meta code");
  var exeFile = platformCodes[0] ? platformCodes[0].textContent.trim() : "Rongyok.Desk_" + cleanVer + "_x64-setup.exe";
  var msiFile = platformCodes[1] ? platformCodes[1].textContent.trim() : "Rongyok.Desk_" + cleanVer + "_x64_en-US.msi";
  var macArmFile = platformCodes[2] ? platformCodes[2].textContent.trim() : "Rongyok.Desk_" + cleanVer + "_aarch64.dmg";
  var macX64File = platformCodes[3] ? platformCodes[3].textContent.trim() : "Rongyok.Desk_" + cleanVer + "_x64.dmg";
  var flatpakFile = platformCodes[4] ? platformCodes[4].textContent.trim() : "Rongyok-Desk-" + cleanVer + "-x86_64.flatpak";

  var snapshotRecords = [];
  function captureAndApply(el, thaiTokens, mode, stopNode) {
    if (!el) return;
    var savedNodes = [];
    for (var child = el.firstChild; child && child !== stopNode; child = child.nextSibling) {
      savedNodes.push(child.cloneNode(true));
    }
    snapshotRecords.push({
      el: el,
      enNodes: savedNodes,
      thTokens: thaiTokens,
      mode: mode || "replace",
      stopNode: stopNode || null
    });
  }

  function captureText(el, thaiText) {
    if (!el) return;
    captureAndApply(el, [thaiText], "replace");
  }

  var metaDescEl = document.querySelector('meta[name="description"]');
  var enTitle = document.title;
  var enMetaDesc = metaDescEl ? metaDescEl.getAttribute("content") || "" : "";

  var skipLink = document.querySelector(".skip-link");
  captureText(skipLink, "ข้ามไปยังเนื้อหาหลัก");

  var primaryLinks = document.querySelectorAll("#primary-nav a");
  var mobileLinks = document.querySelectorAll("#mobile-nav a");
  var navThai = ["ฟีเจอร์", "แพลตฟอร์ม", "วิธีติดตั้ง", "คีย์ลัด", "ความเป็นส่วนตัว"];
  navThai.forEach(function (label, idx) {
    captureText(primaryLinks[idx], label);
    captureText(mobileLinks[idx], label);
  });

  captureText(document.querySelector(".hero-badge span:last-child"), "เวอร์ชัน " + cleanVer + " · Rust + Tauri v2 · Windows, macOS และ Linux");
  captureAndApply(document.querySelector(".hero-copy h1"), [
    "แอปเดสก์ท็อปเฉพาะทางสำหรับ ",
    { tag: "span", className: "accent-text", text: "rongyok.com" }
  ]);
  captureAndApply(document.querySelector(".hero-lead"), [
    "Rongyok Desk คือแอปเบราว์เซอร์เฉพาะเว็บเดียว (SSB) ที่เปิดหน้า ",
    { tag: "code", text: "https://rongyok.com/" },
    " ในหน้าต่างขนาด 1280×800 ที่เรียบง่าย — ล็อกโดเมนอย่างเข้มงวด จำสถานะล็อกอิน จำตำแหน่งหน้าต่าง และเล่นวิดีโอ HTML5 ได้ลื่นไหล เนื่องจากใช้ WebView ของระบบโดยไม่มีหน้าต่างเบราว์เซอร์หรือส่วนขยาย จึงประหยัด RAM กว่าเบราว์เซอร์ทั่วไปแบบ 1 แท็บเปล่า ",
    { tag: "strong", text: "~1.6×–1.8×" },
    " (~157 MB เทียบกับ ~255–275 MB Private Working Set บน Windows Edge/Brave) และประหยัดกว่าการเปิดครั้งแรก ",
    { tag: "strong", text: "~2.2×–4.3×" },
    " (~163 MB เทียบกับ ~355–703 MB)"
  ]);
  captureText(document.querySelector(".hero-actions .btn-primary span"), "แพลตฟอร์มที่รองรับ");
  captureText(document.querySelector(".hero-actions .btn-secondary span"), "คู่มือการติดตั้ง");

  var specTerms = document.querySelectorAll(".hero-specs dt");
  captureText(specTerms[0], "รหัสแอป");
  captureText(specTerms[1], "ตัวติดตั้ง");
  captureText(specTerms[2], "ขนาดแพ็กเกจ");

  var featHeader = document.querySelector("#features .section-header");
  if (featHeader) {
    captureText(featHeader.querySelector(".section-eyebrow"), "ออกแบบเพื่อความโฟกัส");
    captureText(featHeader.querySelector("h2"), "ใช้งานเหมือนแอปเดสก์ท็อปแท้ ไม่มีแท็บเบราว์เซอร์กวนใจ");
    captureAndApply(featHeader.querySelector(".section-subtitle"), [
      "ทุกส่วนของ ",
      { tag: "code", text: "rongyok-ssb" },
      " พัฒนาด้วย Rust และ Tauri v2 (",
      { tag: "code", text: "wry" },
      ") เพื่อให้ใช้งาน Rongyok ได้ไว ปลอดภัย และรองรับการเล่นสื่อเต็มประสิทธิภาพ"
    ]);
  }

  var featureCards = document.querySelectorAll("#features .feature-card");
  var thFeatures = [
    [
      "ล็อกโดเมนอย่างเข้มงวด",
      [
        "อนุญาตเฉพาะ ",
        { tag: "code", text: "https://" },
        " และ ",
        { tag: "code", text: "http://" },
        " ของ ",
        { tag: "code", text: "rongyok.com" },
        " และ ",
        { tag: "code", text: "*.rongyok.com" },
        " (รวมถึง URL ภายในอย่าง ",
        { tag: "code", text: "about:" },
        ", ",
        { tag: "code", text: "data:" },
        ", ",
        { tag: "code", text: "blob:" },
        ") พร้อมกันโดเมนเลียนแบบและ scheme ภายนอกอย่าง ",
        { tag: "code", text: "mailto:" },
        ", ",
        { tag: "code", text: "tel:" },
        " และ ",
        { tag: "code", text: "file:" }
      ]
    ],
    [
      "ส่งลิงก์ภายนอกออกเบราว์เซอร์",
      [
        "ลิงก์นอกโดเมนและป๊อปอัป ",
        { tag: "code", text: 'target="_blank"' },
        " จะถูกเปิดในเบราว์เซอร์หลักของระบบอัตโนมัติ (",
        { tag: "code", text: "ShellExecuteW" },
        " บน Windows, ",
        { tag: "code", text: "open" },
        " บน macOS, ",
        { tag: "code", text: "xdg-open" },
        " บน Linux) ส่วนป๊อปอัปในเว็บเดียวจะเปิดในหน้าต่างเดิม"
      ]
    ],
    [
      "จำสถานะล็อกอินและคุกกี้",
      [
        "เก็บแคชและคุกกี้ลงดิสก์อย่างถาวรในโฟลเดอร์ข้อมูลแอป (",
        { tag: "code", text: "<app_local_data_dir>/webview" },
        " หรือ ",
        { tag: "code", text: "~/.var/app/com.rongyok.ssb" },
        " บน Flatpak) ไม่ต้องล็อกอินซ้ำทุกครั้ง"
      ]
    ],
    [
      "จำตำแหน่งและขนาดหน้าต่าง",
      [
        "บันทึกพิกัด ขนาด (เริ่มต้น ",
        { tag: "code", text: "1280×800" },
        ", ขั้นต่ำ ",
        { tag: "code", text: "520×400" },
        ") และสถานะเต็มจอไว้ใน ",
        { tag: "code", text: "window-placement.txt" },
        " พร้อมตรวจสอบจอที่เชื่อมต่อตอนเปิดและจัดกึ่งกลางให้อัตโนมัติหากถอดจอนอกออก"
      ]
    ],
    [
      "เล่นวิดีโอ HTML5 และเต็มจอลื่นไหล",
      [
        "ตั้งค่า ",
        { tag: "code", text: "--autoplay-policy=no-user-gesture-required" },
        " และปิด background throttling เพื่อการเล่นที่ต่อเนื่อง พร้อมซิงก์โหมดเต็มจอบน Windows WebView2 และเปิดใช้ PulseAudio + DRI ใน Flatpak"
      ]
    ],
    [
      "คีย์ลัดเสถียรและระบบอัปเดตในตัว",
      [
        "ตัวดักคีย์ลัดแบบ capture-phase ช่วยให้กดนำทางได้แม้สคริปต์หน้าเว็บพยายามดัก, ปิด DevTools ในรุ่นเผยแพร่, รองรับการตรวจเช็กอัปเดตในเมนู Help และมีแฟล็ก ",
        { tag: "code", text: "--screenshot <file.png>" },
        " บน Windows"
      ]
    ]
  ];
  thFeatures.forEach(function (item, idx) {
    if (!featureCards[idx]) return;
    captureText(featureCards[idx].querySelector("h3"), item[0]);
    captureAndApply(featureCards[idx].querySelector("p"), item[1]);
  });

  var dlHeader = document.querySelector("#downloads .section-header");
  if (dlHeader) {
    captureText(dlHeader.querySelector(".section-eyebrow"), "เวอร์ชัน " + cleanVer);
    captureText(dlHeader.querySelector("h2"), "แพลตฟอร์มและตัวติดตั้งที่รองรับ");
    captureText(dlHeader.querySelector(".section-subtitle"), "แพ็กเกจตัวติดตั้งสำหรับเดสก์ท็อปของ Rongyok Desk v" + cleanVer + " บน Windows, macOS และ Linux");
  }

  var platformCards = document.querySelectorAll("#downloads .platform-card");
  var thCards = [
    ["แนะนำ", "ตัวติดตั้ง NSIS (", ".exe", ")", "ติดตั้งเฉพาะผู้ใช้ปัจจุบัน และดาวน์โหลด Microsoft Edge WebView2 Runtime ให้อัตโนมัติหากยังไม่มีในเครื่อง", "ดาวน์โหลด .exe"],
    ["MSI", "Windows Installer (", ".msi", ")", "แพ็กเกจมาตรฐาน .msi สำหรับติดตั้งระดับระบบหรือการจัดการแบบองค์กรบน Windows 64-bit", "ดาวน์โหลด .msi"],
    ["arm64", "Disk Image (", ".dmg", ")", "รุ่น Native Apple Silicon (M1–M4) สำหรับ macOS 10.15 ขึ้นไป", "ดาวน์โหลด .dmg (Apple Silicon)"],
    ["x64", "Disk Image (", ".dmg", ")", "รุ่น Intel x86_64 สำหรับ macOS 10.15 ขึ้นไป", "ดาวน์โหลด .dmg (Intel)"],
    ["Flatpak", "แพ็กเกจ Flatpak (", ".flatpak", ")", "แพ็กเกจแบบ sandbox บนพื้นฐาน org.gnome.Platform 49 พร้อม PulseAudio และ DRI", "ดาวน์โหลด .flatpak"]
  ];
  thCards.forEach(function (item, idx) {
    var card = platformCards[idx];
    if (!card) return;
    captureText(card.querySelector(".pill-tag, .pill-muted"), item[0]);
    captureAndApply(card.querySelector("h3"), [item[1], { tag: "code", text: item[2] }, item[3]]);
    if (idx === 4) {
      captureAndApply(card.querySelector(".platform-desc"), [
        "แพ็กเกจแบบ sandbox ที่อิง ",
        { tag: "code", text: "org.gnome.Platform" },
        " 49 พร้อมเปิดใช้ PulseAudio และ DRI"
      ]);
    } else {
      captureText(card.querySelector(".platform-desc"), item[4]);
    }
    var dts = card.querySelectorAll(".asset-meta dt");
    captureText(dts[0], "ไฟล์");
    captureText(dts[1], "ขนาด");
    captureText(card.querySelector(".btn-block"), item[5]);
  });

  var instHeader = document.querySelector("#install .section-header");
  if (instHeader) {
    captureText(instHeader.querySelector(".section-eyebrow"), "ขั้นตอนติดตั้ง");
    captureText(instHeader.querySelector("h2"), "วิธีติดตั้งแยกตามระบบปฏิบัติการ");
    captureText(instHeader.querySelector(".section-subtitle"), "เลือกระบบปฏิบัติการด้านล่างเพื่อดูขั้นตอนติดตั้งและคำแนะนำเมื่อเปิดใช้งาน v" + cleanVer + " ครั้งแรก");
  }

  var winItems = document.querySelectorAll("#panel-windows li");
  captureAndApply(winItems[0], [
    { tag: "strong", text: "เลือกไฟล์ตัวติดตั้ง:" },
    " ใช้ ",
    { tag: "code", text: exeFile },
    " (ตัวติดตั้งเฉพาะผู้ใช้ที่แนะนำ) หรือ ",
    { tag: "code", text: msiFile }
  ]);
  captureAndApply(winItems[1], [
    { tag: "strong", text: "เปิดรันตัวติดตั้ง:" },
    " ตัวติดตั้ง NSIS (",
    { tag: "code", text: ".exe" },
    ") จะดาวน์โหลด Microsoft Edge WebView2 Runtime ให้อัตโนมัติหากเครื่องยังไม่มี"
  ]);
  captureAndApply(winItems[2], [
    { tag: "strong", text: "คำเตือน SmartScreen ครั้งแรก:" },
    " เนื่องจากไฟล์ ",
    { tag: "code", text: rawVer },
    " ยังไม่ได้ลงนามดิจิทัล Windows SmartScreen อาจแจ้งเตือน ให้กด ",
    { tag: "strong", text: "More info" },
    " แล้วเลือก ",
    { tag: "strong", text: "Run anyway" }
  ]);

  var macItems = document.querySelectorAll("#panel-macos li");
  captureAndApply(macItems[0], [
    { tag: "strong", text: "ดาวน์โหลดไฟล์ให้ตรงรุ่น:" },
    " เลือก ",
    { tag: "code", text: macArmFile },
    " สำหรับ Apple Silicon (M1–M4) หรือ ",
    { tag: "code", text: macX64File },
    " สำหรับ Intel Mac (macOS 10.15+)"
  ]);
  captureAndApply(macItems[1], [
    { tag: "strong", text: "ติดตั้งลง Applications:" },
    " เปิดไฟล์ ",
    { tag: "code", text: ".dmg" },
    " แล้วลาก ",
    { tag: "strong", text: "Rongyok Desk.app" },
    " ไปวางในโฟลเดอร์ ",
    { tag: "strong", text: "Applications" }
  ]);
  captureAndApply(macItems[2], [
    { tag: "strong", text: "กดอนุญาต Gatekeeper ครั้งแรก:" },
    " เนื่องจากแอปยังไม่ได้ลงนาม ให้คลิกขวา (หรือ Control-click) ที่ ",
    { tag: "strong", text: "Rongyok Desk.app" },
    " → ",
    { tag: "strong", text: "Open" },
    " → ",
    { tag: "strong", text: "Open" },
    " (หรือไปกดอนุญาตที่ ",
    { tag: "strong", text: "System Settings → Privacy & Security → Open Anyway" },
    ")"
  ]);

  var linuxItems = document.querySelectorAll("#panel-linux li");
  if (linuxItems[0]) {
    var block0 = linuxItems[0].querySelector(".code-block");
    captureAndApply(linuxItems[0], [
      { tag: "strong", text: "ตั้งค่า Flathub ให้พร้อม" },
      " (จำเป็นสำหรับดึงรันไทม์ ",
      { tag: "code", text: "org.gnome.Platform//49" },
      "):"
    ], "leading", block0);
  }
  if (linuxItems[1]) {
    var block1 = linuxItems[1].querySelector(".code-block");
    captureAndApply(linuxItems[1], [
      { tag: "strong", text: "ติดตั้งแพ็กเกจ Flatpak ที่ดาวน์โหลด" },
      " (",
      { tag: "code", text: flatpakFile },
      "):"
    ], "leading", block1);
  }
  if (linuxItems[2]) {
    var block2 = linuxItems[2].querySelector(".code-block");
    captureAndApply(linuxItems[2], [
      { tag: "strong", text: "เปิดใช้งาน Rongyok Desk" },
      " จากเมนูแอปพลิเคชันหรือสั่งรัน:"
    ], "leading", block2);
  }

  var scHeader = document.querySelector("#shortcuts .section-header");
  if (scHeader) {
    captureText(scHeader.querySelector(".section-eyebrow"), "การควบคุม");
    captureText(scHeader.querySelector("h2"), "คีย์ลัดและเมนูบนเดสก์ท็อป");
    captureAndApply(scHeader.querySelector(".section-subtitle"), [
      "คำสั่งทั้งหมดอยู่ในเมนู ",
      { tag: "strong", text: "Navigate" },
      " และ ",
      { tag: "strong", text: "Help" },
      " พร้อมตัวดักคีย์ลัดแบบ capture-phase เพื่อให้กดใช้งานได้เสถียรทุกหน้า"
    ]);
  }

  var scHeads = document.querySelectorAll(".shortcuts-table thead th");
  captureText(scHeads[0], "คำสั่ง");
  captureText(scHeads[1], "คีย์ลัด / เมนู");
  captureText(scHeads[2], "การทำงาน");

  var scRows = document.querySelectorAll(".shortcuts-table tbody tr");
  var thRows = [
    ["ย้อนกลับ", null, ["ย้อนกลับไปยังหน้าก่อนหน้าในประวัติ"]],
    ["ไปข้างหน้า", null, ["เลื่อนไปหน้าถัดไปในประวัติ"]],
    ["โหลดหน้าใหม่", null, ["รีโหลดหน้า Rongyok ปัจจุบัน"]],
    ["กลับหน้าหลัก rongyok.com", null, ["เปิดตรงกลับไปยัง ", { tag: "code", text: "https://rongyok.com/" }]],
    ["เต็มหน้าจอ", null, ["สลับโหมดเต็มหน้าจอบนเดสก์ท็อป"]],
    ["เปิดหน้านี้ในเบราว์เซอร์", [{ tag: "code", text: "Navigate → Open this page in browser" }], ["ส่ง URL หน้าปัจจุบันไปเปิดในเบราว์เซอร์หลัก"]],
    ["หน้าต่างคีย์ลัด (Keyboard Shortcuts)", null, ["เปิดหน้าต่างแนะนำคีย์ลัด (มีในเมนู ", { tag: "strong", text: "Help" }, " และเปิดอัตโนมัติครั้งแรก)"]],
    ["ตรวจอัปเดต (Check for Updates...)", [{ tag: "code", text: "Help → Check for Updates..." }], ["ตรวจสอบเวอร์ชันใหม่จาก GitHub Releases"]],
    ["เกี่ยวกับแอป (About Rongyok Desk)", [{ tag: "code", text: "Help → About Rongyok Desk" }], ["แสดงเวอร์ชัน ระบบปฏิบัติการ/สถาปัตยกรรม และลิขสิทธิ์"]],
    ["ออกจากแอป", null, ["บันทึกตำแหน่งหน้าต่างแล้วปิด Rongyok Desk"]]
  ];
  thRows.forEach(function (rowSpec, idx) {
    var tr = scRows[idx];
    if (!tr) return;
    var cells = tr.querySelectorAll("td");
    captureText(cells[0], rowSpec[0]);
    if (rowSpec[1]) {
      captureAndApply(cells[1], rowSpec[1]);
    }
    captureAndApply(cells[2], rowSpec[2]);
  });

  var privHeader = document.querySelector("#privacy .section-header");
  if (privHeader) {
    captureText(privHeader.querySelector(".section-eyebrow"), "ความโปร่งใส");
    captureText(privHeader.querySelector("h2"), "ความเป็นส่วนตัวและความปลอดภัย");
    captureAndApply(privHeader.querySelector(".section-subtitle"), [
      "Rongyok Desk แยกขอบเขตระหว่างตัวแอปกับเว็บไซต์ ",
      { tag: "code", text: "rongyok.com" },
      " อย่างชัดเจน และไม่มีการติดตามจากตัวแอป"
    ]);
  }

  var privCards = document.querySelectorAll("#privacy .feature-card");
  var thPrivacy = [
    [
      "ตัวแอปไม่มีการติดตาม (No App Telemetry)",
      [
        "Rongyok Desk ไม่เก็บสถิติการใช้งาน รายงานข้อผิดพลาด หรือข้อมูลส่วนตัวใดๆ การเชื่อมต่อออกนอก ",
        { tag: "code", text: "rongyok.com" },
        " ที่เกิดจากตัวแอปมีเพียงการตรวจสอบไฟล์ ",
        { tag: "code", text: "latest.json" },
        " บน GitHub Releases เพื่อเช็กอัปเดตเท่านั้น (จำกัดเฉพาะคลัง ",
        { tag: "code", text: "EEMEEMMEEx/rongyok-desk" },
        ")"
      ]
    ],
    [
      "แยกขอบเขตตัวแอปกับเว็บไซต์ปลายทาง",
      [
        "ไฟล์ทั้งหมดถูกเก็บไว้เฉพาะบนเครื่องของคุณ (",
        { tag: "code", text: "<app_local_data_dir>" },
        " สำหรับเซสชัน WebView, ",
        { tag: "code", text: "window-placement.txt" },
        " และ ",
        { tag: "code", text: "onboarding-seen.txt" },
        ") ส่วนคุกกี้หรือการติดตามระดับหน้าเว็บจะมาจากเว็บไซต์ ",
        { tag: "code", text: "rongyok.com" },
        " เองเช่นเดียวกับการเปิดผ่านเบราว์เซอร์ปกติ"
      ]
    ],
    [
      "ไม่เปิดสิทธิ์ API ระดับเครื่องให้หน้าเว็บ",
      [
        "ปิด ",
        { tag: "code", text: "withGlobalTauri" },
        " และกำหนด Tauri IPC capabilities เป็น ",
        { tag: "code", text: "{}" },
        " หน้าเว็บจึงไม่สามารถเรียกคำสั่งระบบหรืออ่านไฟล์ในเครื่องได้ และรุ่น Linux Flatpak ไม่ได้ขอสิทธิ์เข้าถึงโฟลเดอร์ Home"
      ]
    ],
    [
      "ล็อกโดเมนและตรวจสอบลายเซ็นอัปเดต",
      [
        "โหลดเฉพาะ ",
        { tag: "code", text: "rongyok.com" },
        " และ ",
        { tag: "code", text: "*.rongyok.com" },
        " ภายในแอป ลิงก์ภายนอกจะถูกส่งไปเปิดในเบราว์เซอร์หลัก และการอัปเดตภายในแอปจะตรวจสอบลายเซ็นดิจิทัลก่อนติดตั้ง"
      ]
    ]
  ];
  thPrivacy.forEach(function (item, idx) {
    if (!privCards[idx]) return;
    captureText(privCards[idx].querySelector("h3"), item[0]);
    captureAndApply(privCards[idx].querySelector("p"), item[1]);
  });

  var footerSub = document.querySelector(".footer-brand span");
  captureAndApply(footerSub, [
    "Package ID ",
    { tag: "code", text: "com.rongyok.ssb" },
    " · พัฒนาด้วย Rust & Tauri v2"
  ]);
  var footerLinks = document.querySelectorAll(".footer-links a");
  captureText(footerLinks[0], "ฟีเจอร์");
  captureText(footerLinks[1], "แพลตฟอร์ม");
  captureText(footerLinks[2], "วิธีติดตั้ง");
  captureText(footerLinks[3], "ความเป็นส่วนตัว");

  function applyLanguage(lang) {
    var next = lang === "th" ? "th" : "en";
    var isTh = next === "th";
    root.setAttribute("data-lang", next);
    root.setAttribute("lang", next);

    document.title = isTh
      ? "Rongyok Desk — แอปเดสก์ท็อปเฉพาะทางสำหรับ rongyok.com"
      : enTitle;
    if (metaDescEl) {
      metaDescEl.setAttribute(
        "content",
        isTh
          ? "Rongyok Desk v" + cleanVer + " แอปเดสก์ท็อปเฉพาะเว็บเดียวสำหรับ rongyok.com พัฒนาด้วย Rust และ Tauri v2 รองรับ Windows, macOS และ Linux"
          : enMetaDesc
      );
    }

    snapshotRecords.forEach(function (rec) {
      if (isTh) {
        if (rec.mode === "leading") {
          setLeadingMixedContent(rec.el, rec.stopNode, rec.thTokens);
        } else {
          setMixedContent(rec.el, rec.thTokens);
        }
      } else {
        if (rec.mode === "leading") {
          while (rec.el.firstChild && rec.el.firstChild !== rec.stopNode) {
            rec.el.removeChild(rec.el.firstChild);
          }
          rec.enNodes.forEach(function (node) {
            rec.el.insertBefore(node.cloneNode(true), rec.stopNode);
          });
        } else {
          while (rec.el.firstChild) {
            rec.el.removeChild(rec.el.firstChild);
          }
          rec.enNodes.forEach(function (node) {
            rec.el.appendChild(node.cloneNode(true));
          });
        }
      }
    });

    copyButtons.forEach(function (btn) {
      var label = btn.querySelector(".copy-label");
      if (label) {
        label.textContent = btn.classList.contains("is-copied")
          ? isTh
            ? "คัดลอกแล้ว"
            : "Copied"
          : isTh
            ? "คัดลอก"
            : "Copy";
      }
    });

    if (langToggle) {
      langToggle.setAttribute("aria-pressed", String(isTh));
      langToggle.setAttribute(
        "aria-label",
        isTh ? "Switch language to English" : "สลับภาษาเป็นภาษาไทย"
      );
      langToggle.setAttribute(
        "title",
        isTh ? "สลับภาษา (TH / EN)" : "Switch language (TH / EN)"
      );
    }
    if (langLabel) {
      langLabel.textContent = isTh ? "EN" : "TH";
    }
    applyTheme(root.getAttribute("data-theme") || "dark");
  }

  applyLanguage(getCurrentLang());

  if (langToggle) {
    langToggle.addEventListener("click", function () {
      var next = getCurrentLang() === "th" ? "en" : "th";
      applyLanguage(next);
      saveSetting(LANG_KEY, next);
    });
  }
})();
