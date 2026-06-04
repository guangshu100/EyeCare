// Autostart plugin - loaded dynamically with fallback
let enableAutostart = async () => {};
let disableAutostart = async () => {};
let isAutostartEnabled = async () => false;

try {
  const autostartModule = await import('@tauri-apps/plugin-autostart');
  enableAutostart = autostartModule.enable;
  disableAutostart = autostartModule.disable;
  isAutostartEnabled = autostartModule.isEnabled;
} catch (e) {
  console.warn('Autostart plugin not available:', e);
}

// Tauri API - with fallback
const getTauriApi = () => {
  if (!window.__TAURI__) {
    console.error('Tauri API not available');
    return null;
  }
  return window.__TAURI__;
};

const invoke = async (...args) => {
  const tauri = getTauriApi();
  if (!tauri) throw new Error('Tauri API not available');
  return tauri.core.invoke(...args);
};

const listen = async (...args) => {
  const tauri = getTauriApi();
  if (!tauri) throw new Error('Tauri API not available');
  return tauri.event.listen(...args);
};

// State
let config = null;
let isPaused = false;
let eyeHealth = 100;
let severity = 1;
let continuousWorkSeconds = 0;
let nextReminderSeconds = 0;

// Water state
let waterConfig = null;
let waterNextReminder = 0;

// Pet state
let petData = null;

// Initialize
window.addEventListener("DOMContentLoaded", async () => {
  try {
    await loadConfig();
    await loadWaterConfig();
    await loadMedicationConfig();
    await loadPetState();
    setupEventListeners();
    setupIdleListener();
    setupNotificationListener();
    setupFullscreenListener();
    setupResumeListener();
    setupWaterNotificationListener();
    setupMedicationListeners();
    updateStatus();
    startLocalCountdown();
    startWaterCountdown();
    startMedicationCountdown();
  } catch (e) {
    console.error("Failed to initialize:", e);
  }
});

async function loadConfig() {
  try {
    config = await invoke("get_config");
    isPaused = !config.run_in_tray;
    applyConfigToUI();
    
    // Initialize autostart based on saved config
    try {
      const currentlyEnabled = await isAutostartEnabled();
      if (config.auto_start && !currentlyEnabled) {
        await enableAutostart();
      } else if (!config.auto_start && currentlyEnabled) {
        await disableAutostart();
      }
    } catch (e) {
      console.error("Failed to sync autostart state:", e);
    }
  } catch (e) {
    console.error("Failed to load config:", e);
  }
}

function applyConfigToUI() {
  if (!config) return;
  
  document.getElementById("idle-threshold").value = config.idle_threshold;
  document.getElementById("idle-threshold-value").textContent = `${config.idle_threshold}分钟`;
  
  document.getElementById("break-duration").value = config.break_duration;
  document.getElementById("break-duration-value").textContent = `${config.break_duration}秒`;
  
  document.getElementById("max-skips").value = config.max_skips_per_day;
  document.getElementById("max-skips-value").textContent = `${config.max_skips_per_day}次`;
  
  document.getElementById("auto-start").checked = config.auto_start;
  
  // Theme
  document.querySelectorAll(".theme-btn").forEach(btn => {
    btn.classList.toggle("active", btn.dataset.color === config.theme_color);
  });
  
  // AI
  document.getElementById("ai-enabled").checked = config.ai?.enabled || false;
  toggleAiConfig(config.ai?.enabled || false);
  
  if (config.ai) {
    document.getElementById("ai-provider").value = config.ai.provider || "siliconflow";
    document.getElementById("api-base-url").value = config.ai.api_base_url || "";
    document.getElementById("api-key").value = config.ai.api_key || "";
    document.getElementById("model").value = config.ai.model || "";
    document.getElementById("preferred-style").value = config.ai.preferred_style || "balanced";
    updateApiBaseUrlVisibility(config.ai.provider || "siliconflow");
  }
  
  document.getElementById("break-count").textContent = config.break_count_today || 0;

  // 白名单
  whitelistApps = config.whitelist_apps || [];
  renderWhitelist();
}

async function loadPetState() {
  try {
    petData = await invoke("get_pet_state");
    updatePetUI();
  } catch (e) {
    console.error("Failed to load pet state:", e);
  }
}

function updatePetUI() {
  if (!petData) return;
  
  const moodEmoji = petData.mood > 70 ? "😊" : petData.mood > 40 ? "😐" : "😢";
  document.getElementById("header-pet-info").textContent = 
    `${petData.name || "小萌"} ${moodEmoji} ${petData.mood || 100}`;
}

function setupEventListeners() {
  // 基础设置 - 自动保存
  document.getElementById("idle-threshold").addEventListener("input", (e) => {
    document.getElementById("idle-threshold-value").textContent = `${e.target.value}分钟`;
  });
  document.getElementById("idle-threshold").addEventListener("change", saveConfigAuto);
  
  document.getElementById("break-duration").addEventListener("input", (e) => {
    document.getElementById("break-duration-value").textContent = `${e.target.value}秒`;
  });
  document.getElementById("break-duration").addEventListener("change", saveConfigAuto);
  
  document.getElementById("max-skips").addEventListener("input", (e) => {
    document.getElementById("max-skips-value").textContent = `${e.target.value}次`;
  });
  document.getElementById("max-skips").addEventListener("change", saveConfigAuto);
  
  document.getElementById("auto-start").addEventListener("change", saveConfigAuto);
  
  document.querySelectorAll(".theme-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      document.querySelectorAll(".theme-btn").forEach(b => b.classList.remove("active"));
      btn.classList.add("active");
      saveConfigAuto();
    });
  });
  
  // AI
  document.getElementById("ai-enabled").addEventListener("change", (e) => {
    toggleAiConfig(e.target.checked);
    saveConfigAuto();
  });
  
  document.getElementById("ai-provider").addEventListener("change", (e) => {
    updateApiBaseUrlVisibility(e.target.value);
    updateDefaultModel(e.target.value);
    saveConfigAuto();
  });
  
  document.getElementById("api-base-url").addEventListener("change", saveConfigAuto);
  document.getElementById("api-key").addEventListener("change", saveConfigAuto);
  document.getElementById("model").addEventListener("change", saveConfigAuto);
  document.getElementById("preferred-style").addEventListener("change", saveConfigAuto);
  
  // 测试AI连接
  document.getElementById("test-ai-btn").addEventListener("click", testAiConnection);
  
  // API密钥明文切换
  document.getElementById("toggle-api-key").addEventListener("click", () => {
    const input = document.getElementById("api-key");
    const btn = document.getElementById("toggle-api-key");
    if (input.type === "password") {
      input.type = "text";
      btn.textContent = "🙈";
    } else {
      input.type = "password";
      btn.textContent = "👁️";
    }
  });

  // 白名单管理
  initWhitelistSuggestions();

  // 按钮
  document.getElementById("toggle-pause").addEventListener("click", togglePause);
  document.getElementById("test-break").addEventListener("click", testBreak);
  document.getElementById("drink-water-btn").addEventListener("click", drinkWater);

  // 宠物互动
  document.getElementById("pet-avatar-header").addEventListener("click", petInteract);

  // 喝水设置
  document.getElementById("water-enabled").addEventListener("change", (e) => {
    toggleWaterConfig(e.target.checked);
    saveWaterConfig();
  });
  
  document.getElementById("water-interval").addEventListener("input", (e) => {
    document.getElementById("water-interval-value").textContent = `${e.target.value}分钟`;
  });
  document.getElementById("water-interval").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-goal").addEventListener("input", (e) => {
    document.getElementById("water-goal-value").textContent = `${e.target.value}ml`;
    document.getElementById("water-goal-display").textContent = e.target.value;
  });
  document.getElementById("water-goal").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-cup").addEventListener("input", (e) => {
    document.getElementById("water-cup-value").textContent = `${e.target.value}ml`;
  });
  document.getElementById("water-cup").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-cup-general").addEventListener("input", (e) => {
    document.getElementById("water-cup-general-value").textContent = `${e.target.value}ml`;
    document.getElementById("water-cup").value = e.target.value;
    document.getElementById("water-cup-value").textContent = `${e.target.value}ml`;
  });
  document.getElementById("water-cup-general").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-start-hour").addEventListener("change", saveWaterConfig);
  document.getElementById("water-end-hour").addEventListener("change", saveWaterConfig);

  // 用药提醒
  document.getElementById("medication-enabled").addEventListener("change", () => {
    toggleMedicationConfig(document.getElementById("medication-enabled").checked);
    saveMedicationConfig();
  });
  document.getElementById("med-escalation").addEventListener("input", (e) => {
    document.getElementById("med-escalation-value").textContent = `${e.target.value}分钟`;
  });
  document.getElementById("med-escalation").addEventListener("change", saveMedicationConfig);
  document.getElementById("med-notify").addEventListener("change", saveMedicationConfig);
  document.getElementById("med-confirm").addEventListener("change", saveMedicationConfig);
  document.getElementById("med-form").addEventListener("submit", onMedicationFormSubmit);
  document.getElementById("med-cancel-btn").addEventListener("click", resetMedicationForm);
}

function toggleAiConfig(show) {
  const aiConfig = document.getElementById("ai-config");
  aiConfig.classList.toggle("hidden", !show);
}

function updateApiBaseUrlVisibility(provider) {
  const row = document.getElementById("api-base-url-row");
  const defaultUrls = {
    siliconflow: "https://api.siliconflow.cn/v1",
    openai: "https://api.openai.com/v1",
    deepseek: "https://api.deepseek.com/v1",
    ollama: "http://localhost:11434/v1",
    custom: ""
  };
  
  if (provider === "custom") {
    row.style.display = "flex";
    document.getElementById("api-base-url").placeholder = "输入API地址";
  } else {
    row.style.display = "none";
    document.getElementById("api-base-url").value = defaultUrls[provider] || "";
  }
}

// 白名单管理
let whitelistApps = [];

// 应用图标映射表
const APP_ICON_MAP = {
  "腾讯会议": "📹",
  "zoom": "🎥",
  "teams": "💬",
  "skype": "📞",
  "钉钉": "📌",
  "飞书": "📄",
  "腾讯QQ": "🐧",
  "微信": "💚",
  "企业微信": "🏢",
  "slack": "💬",
  "webex": "🌐",
  "go to meeting": "🤝",
  "google meet": "📅",
  "notion": "📓",
  "obsidian": "💜",
  "figma": "🎨",
  "photoshop": "🖼️",
  "illustrator": "✏️",
  "premiere": "🎬",
  "after effects": "✨",
  "blender": "🧊",
  "unity": "🎮",
  "unreal": "🎯",
  "visual studio": "💻",
  "visual studio code": "📝",
  "vs code": "📝",
  "code": "📝",
  "intellij": "🧠",
  "idea": "🧠",
  "pycharm": "🐍",
  "webstorm": "🌊",
  "clion": "🔧",
  "goland": "🐹",
  "rustrover": "🦀",
  "cursor": "🖱️",
  "vim": "📏",
  "neovim": "📝",
  "terminal": "⬛",
  "powershell": "⚡",
  "cmd": "⬛",
  "chrome": "🌐",
  "edge": "🔵",
  "firefox": "🦊",
  "safari": "🧭",
  "bilibili": "📺",
  "youtube": "▶️",
  "netflix": "🎬",
  "spotify": "🎵",
  "qq音乐": "🎵",
  "网易云音乐": "🎵",
  " steam": "🎮",
  "epic games": "🎮",
};

// 推荐应用列表（按分类）
const SUGGESTED_APPS = [
  { category: "视频会议", apps: [
    { name: "腾讯会议", icon: "📹" },
    { name: "Zoom", icon: "🎥" },
    { name: "Teams", icon: "💬" },
    { name: "Skype", icon: "📞" },
    { name: "钉钉", icon: "📌" },
    { name: "飞书", icon: "📄" },
    { name: "Webex", icon: "🌐" },
    { name: "Google Meet", icon: "📅" },
  ]},
  { category: "即时通讯", apps: [
    { name: "微信", icon: "💚" },
    { name: "腾讯QQ", icon: "🐧" },
    { name: "企业微信", icon: "🏢" },
    { name: "Slack", icon: "💬" },
  ]},
  { category: "开发工具", apps: [
    { name: "Visual Studio Code", icon: "📝" },
    { name: "IntelliJ IDEA", icon: "🧠" },
    { name: "PyCharm", icon: "🐍" },
    { name: "Cursor", icon: "🖱️" },
    { name: "Terminal", icon: "⬛" },
  ]},
  { category: "设计创作", apps: [
    { name: "Figma", icon: "🎨" },
    { name: "Photoshop", icon: "🖼️" },
    { name: "Premiere", icon: "🎬" },
    { name: "Blender", icon: "🧊" },
  ]},
  { category: "影音娱乐", apps: [
    { name: "Bilibili", icon: "📺" },
    { name: "YouTube", icon: "▶️" },
    { name: "Spotify", icon: "🎵" },
    { name: "网易云音乐", icon: "🎵" },
    { name: "Steam", icon: "🎮" },
  ]},
  { category: "浏览器", apps: [
    { name: "Chrome", icon: "🌐" },
    { name: "Edge", icon: "🔵" },
    { name: "Firefox", icon: "🦊" },
  ]},
];

// 获取应用图标
function getAppIcon(appName) {
  const lower = appName.toLowerCase();
  // 先精确匹配
  if (APP_ICON_MAP[appName]) return APP_ICON_MAP[appName];
  if (APP_ICON_MAP[lower]) return APP_ICON_MAP[lower];
  // 模糊匹配
  for (const [key, icon] of Object.entries(APP_ICON_MAP)) {
    if (lower.includes(key) || key.includes(lower)) return icon;
  }
  return "📱";
}

function renderWhitelist() {
  const container = document.getElementById("whitelist-apps");
  if (whitelistApps.length === 0) {
    container.innerHTML = '<span class="whitelist-empty-hint">暂无白名单应用，推荐添加会议和通讯类应用</span>';
    return;
  }
  container.innerHTML = whitelistApps.map((app, index) => `
    <span class="whitelist-tag" data-index="${index}" title="${escapeHtml(app)}">
      <span class="app-name">${escapeHtml(app)}</span>
      <button class="remove-btn" data-remove="${index}">×</button>
    </span>
  `).join("");
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

function escapeAttr(text) {
  return text.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/'/g, "&#39;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function addWhitelistApp(appName) {
  const input = document.getElementById("whitelist-input");
  const name = (appName || input.value).trim();
  if (name && !whitelistApps.some(a => a.toLowerCase() === name.toLowerCase())) {
    whitelistApps.push(name);
    renderWhitelist();
    saveConfigAuto();
  }
  input.value = "";
  hideSuggestions();
}

function removeWhitelistApp(index) {
  whitelistApps.splice(index, 1);
  renderWhitelist();
  saveConfigAuto();
}

// 推荐应用下拉
function showSuggestions(filter = "") {
  const container = document.getElementById("whitelist-suggestions");
  const filterLower = filter.toLowerCase();

  // 过滤掉已添加的
  const addedLower = whitelistApps.map(a => a.toLowerCase());

  let html = "";
  let hasAny = false;

  for (const group of SUGGESTED_APPS) {
    const filteredApps = group.apps.filter(app =>
      !addedLower.includes(app.name.toLowerCase()) &&
      (!filterLower || app.name.toLowerCase().includes(filterLower))
    );
    if (filteredApps.length === 0) continue;
    hasAny = true;
    html += `<div class="suggestions-category-label">${escapeHtml(group.category)}</div>`;
    for (const app of filteredApps) {
      html += `<div class="whitelist-suggestion-item" data-app-name="${escapeAttr(app.name)}">
        <span class="suggestion-icon">${app.icon}</span>
        <span class="suggestion-name">${escapeHtml(app.name)}</span>
        <span class="suggestion-category">${escapeHtml(group.category)}</span>
      </div>`;
    }
  }

  if (!hasAny) {
    html = '<div class="suggestions-category-label" style="padding:12px;text-align:center;">无匹配应用，直接回车添加</div>';
  }

  container.innerHTML = html;
  container.classList.remove("hidden");
}

function hideSuggestions() {
  document.getElementById("whitelist-suggestions").classList.add("hidden");
}

function initWhitelistSuggestions() {
  const input = document.getElementById("whitelist-input");
  const suggestionsEl = document.getElementById("whitelist-suggestions");

  // 点击输入框时显示推荐
  input.addEventListener("focus", () => {
    showSuggestions(input.value.trim());
  });

  // 输入时过滤
  input.addEventListener("input", () => {
    const val = input.value.trim();
    showSuggestions(val);
  });

  // 回车添加
  input.addEventListener("keypress", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addWhitelistApp();
    }
  });

  // 事件委托：点击推荐项添加
  suggestionsEl.addEventListener("mousedown", (e) => {
    const item = e.target.closest(".whitelist-suggestion-item");
    if (item) {
      e.preventDefault(); // 阻止 input 失焦
      const appName = item.getAttribute("data-app-name");
      if (appName) addWhitelistApp(appName);
    }
  });

  // 点击外部关闭
  document.addEventListener("mousedown", (e) => {
    if (!e.target.closest(".whitelist-input-wrap")) {
      hideSuggestions();
    }
  });

  // 事件委托：删除白名单标签
  document.getElementById("whitelist-apps").addEventListener("click", (e) => {
    const btn = e.target.closest(".remove-btn");
    if (btn) {
      const index = parseInt(btn.getAttribute("data-remove"), 10);
      if (!isNaN(index)) removeWhitelistApp(index);
    }
  });
}

function updateDefaultModel(provider) {
  const modelInput = document.getElementById("model");
  const defaults = {
    siliconflow: "Qwen/Qwen2.5-7B-Instruct",
    openai: "gpt-3.5-turbo",
    deepseek: "deepseek-chat",
    ollama: "llama3",
    custom: ""
  };
  modelInput.value = defaults[provider] || "";
}

async function testAiConnection() {
  const resultEl = document.getElementById("ai-test-result");
  const btn = document.getElementById("test-ai-btn");
  
  btn.disabled = true;
  btn.style.opacity = "0.5";
  resultEl.classList.remove("hidden", "success", "error");
  resultEl.textContent = "正在保存配置并连接...";
  
  try {
    // 先保存当前配置
    await saveConfigAuto();
    // 再测试连接（从 state 读取最新配置）
    await invoke("test_api_connection");
    resultEl.classList.add("success");
    resultEl.textContent = "✅ 连接成功！模型响应正常";
  } catch (e) {
    resultEl.classList.add("error");
    resultEl.textContent = `❌ 连接失败: ${e}`;
  }
  
  btn.disabled = false;
  btn.style.opacity = "1";
}

function toggleWaterConfig(show) {
  const waterConfigEl = document.getElementById("water-config");
  waterConfigEl.classList.toggle("hidden", !show);
}

async function saveConfigAuto() {
  if (!config) return;
  
  const autoStartEnabled = document.getElementById("auto-start").checked;
  
  const newConfig = {
    ...config,
    idle_threshold: parseInt(document.getElementById("idle-threshold").value),
    break_duration: parseInt(document.getElementById("break-duration").value),
    max_skips_per_day: parseInt(document.getElementById("max-skips").value),
    auto_start: autoStartEnabled,
    theme_color: document.querySelector(".theme-btn.active")?.dataset.color || "#E8F4F8",
    whitelist_apps: whitelistApps,
    ai: {
      ...config.ai,
      enabled: document.getElementById("ai-enabled").checked,
      provider: document.getElementById("ai-provider").value,
      api_base_url: document.getElementById("api-base-url").value,
      api_key: document.getElementById("api-key").value,
      model: document.getElementById("model").value,
      preferred_style: document.getElementById("preferred-style").value,
    },
  };
  
  try {
    await invoke("save_config", { newConfig });
    config = newConfig;
    
    // Apply autostart setting to system
    try {
      if (autoStartEnabled) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
    } catch (e) {
      console.error("Failed to set autostart:", e);
    }
  } catch (e) {
    console.error("Failed to save config:", e);
  }
}

async function togglePause() {
  isPaused = !isPaused;
  try {
    await invoke("toggle_pause");
  } catch (e) {
    console.error("Failed to toggle pause:", e);
  }
  updateStatus();
}

async function testBreak() {
  try {
    await invoke("show_fullscreen", { 
      forced: false, 
      severity: 1, 
      eye_health: 100,
      skip_history_json: null,
      total_skipped_seconds: 0
    });
  } catch (e) {
    console.error("Failed to test break:", e);
  }
}

async function petInteract() {
  try {
    petData = await invoke("pet_interact");
    updatePetUI();
  } catch (e) {
    console.error("Failed to interact with pet:", e);
  }
}

function updateStatus() {
  const statusText = document.getElementById("status-text");
  const statusDot = document.getElementById("status-dot");
  const toggleBtn = document.getElementById("toggle-pause");

  if (isPaused) {
    statusText.textContent = "已暂停";
    statusDot.classList.remove("active");
    toggleBtn.textContent = "开始";
  } else {
    statusText.textContent = "运行中";
    statusDot.classList.add("active");
    toggleBtn.textContent = "暂停";
  }
}

// ==================== 空闲状态监听 ====================

async function setupIdleListener() {
  await listen("idle-status", (event) => {
    const status = event.payload;
    eyeHealth = status.eye_health || 100;
    severity = status.severity || 1;
    continuousWorkSeconds = status.continuous_work_seconds || 0;
    nextReminderSeconds = status.next_reminder_seconds || 0;

    document.getElementById("eye-health").textContent = `${eyeHealth}%`;
    document.getElementById("break-count").textContent = status.skip_count_today || 0;
    
    // 更新宠物状态
    if (status.pet) {
      petData = status.pet;
      updatePetUI();
    }
  });

  try {
    const status = await invoke("get_status");
    eyeHealth = status.eye_health || 100;
    continuousWorkSeconds = status.continuous_work_seconds || 0;
    nextReminderSeconds = status.next_reminder_seconds || 0;
    
    // 更新 UI
    document.getElementById("eye-health").textContent = `${eyeHealth}%`;
    document.getElementById("break-count").textContent = status.skip_count_today || 0;
  } catch (e) {
    console.error("Failed to get status:", e);
  }
}

function startLocalCountdown() {
  setInterval(() => {
    if (!isPaused) {
      continuousWorkSeconds++;
      const workMinutes = Math.floor(continuousWorkSeconds / 60);
      const workSeconds = continuousWorkSeconds % 60;
      document.getElementById("continuous-work-time").textContent = `${workMinutes}分${workSeconds}秒`;
      // 眼睛休息倒计时每秒递减
      if (nextReminderSeconds > 0) nextReminderSeconds--;
    }
    // 喝水倒计时不受暂停影响，按 1 秒递减
    if (waterConfig && waterConfig.enabled && waterNextReminder > 0) {
      waterNextReminder--;
    }
    // 渲染"接下来"卡片（即使暂停也渲染，让用户看到吃药的固定时点）
    renderUpcoming();
  }, 1000);
}

// ==================== 接下来 卡片渲染 ====================

/**
 * 渲染"接下来"芯片：下一次休息 / 下一次喝水 / 下一次吃药（单行内嵌）
 * - 眼睛休息和喝水用 ⏱ + mm:ss 倒计时（一眼看出"在 N 时间后"）
 * - 吃药用 🕐 + HH:MM 固定时点（一眼看出"几点几分准时"）
 * - < 5min 渐变橙（soon），< 1min 强提醒（urgent）
 */
function renderUpcoming() {
  const restEl = document.getElementById("upcoming-rest");
  const waterEl = document.getElementById("upcoming-water");
  const medEl = document.getElementById("upcoming-medication");
  if (!restEl || !waterEl || !medEl) return;

  // 1) 眼睛休息：暂停时隐藏
  if (isPaused) {
    restEl.classList.add("hidden");
  } else {
    restEl.classList.remove("hidden");
    const restTimeEl = document.getElementById("upcoming-rest-time");
    if (nextReminderSeconds > 0) {
      // ⏱ + mm:ss 倒计时格式：明确"在 N 时间后"
      restTimeEl.textContent = `⏱ ${formatMmSs(nextReminderSeconds)}`;
      applyChipState(restEl, nextReminderSeconds);
    } else {
      restTimeEl.textContent = "⏱ 立即";
      restEl.classList.add("urgent");
      restEl.classList.remove("soon");
    }
  }

  // 2) 喝水：未启用或非活跃时段隐藏
  if (!waterConfig || !waterConfig.enabled) {
    waterEl.classList.add("hidden");
  } else {
    waterEl.classList.remove("hidden");
    const waterTimeEl = document.getElementById("upcoming-water-time");
    const inActive = waterConfig.is_in_active_hours !== false;
    if (!inActive) {
      // ⏹ 明确"暂停中"语义，与"立即"区分
      waterTimeEl.textContent = "⏹ 休眠";
      waterEl.classList.remove("urgent", "soon");
    } else if (waterNextReminder > 0) {
      waterTimeEl.textContent = `⏱ ${formatMmSs(waterNextReminder)}`;
      applyChipState(waterEl, waterNextReminder);
    } else {
      waterTimeEl.textContent = "⏱ 立即";
      waterEl.classList.add("urgent");
      waterEl.classList.remove("soon");
    }
  }

  // 3) 吃药：未启用 或 没有待服药时隐藏
  const medInfo = computeNextMedication();
  if (!medInfo) {
    medEl.classList.add("hidden");
  } else {
    medEl.classList.remove("hidden");
    const medTimeEl = document.getElementById("upcoming-medication-time");
    const medNameEl = document.getElementById("upcoming-medication-name");

    // 名称字段：直接显示药名（替代静态"用药"，一眼看出具体药物）
    if (medNameEl) {
      medNameEl.textContent = medInfo.medicationName || "用药";
    }

    // 时间字段：🕐 时点 + 倒计时（< 5min 时追加"·X分"，明确距离）
    const mins = Math.max(1, Math.round(medInfo.remaining / 60));
    if (medInfo.remaining <= 300) {
      // 5 分钟内：追加倒计时（此时最重要）
      medTimeEl.textContent = `🕐 ${medInfo.scheduledTime} · ${mins}分`;
    } else {
      // 平时：仅时点（保持紧凑，避免 chip 太长）
      medTimeEl.textContent = `🕐 ${medInfo.scheduledTime}`;
    }
    // title 始终显示完整信息
    medEl.title = `下一次吃药 ${medInfo.scheduledTime}（${medInfo.medicationName}，还有约 ${mins} 分钟）`;
    applyChipState(medEl, medInfo.remaining);
  }
}

/** 应用 chip 的紧急/即将状态样式 */
function applyChipState(chipEl, remaining) {
  chipEl.classList.remove("soon", "urgent");
  if (remaining <= 60) {
    chipEl.classList.add("urgent");
  } else if (remaining <= 300) {
    chipEl.classList.add("soon");
  }
}

/** 把秒数格式化为 mm:ss（或 h:mm:ss 当 >= 1 小时） */
function formatMmSs(secs) {
  secs = Math.max(0, Math.floor(secs));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const pad = (n) => String(n).padStart(2, "0");
  if (h > 0) return `${h}:${pad(m)}:${pad(s)}`;
  return `${pad(m)}:${pad(s)}`;
}

/**
 * 从 medicationConfig.today_logs 中找最近的 Pending 时点
 * @returns {{scheduledTime: string, remaining: number, medicationName: string} | null}
 */
function computeNextMedication() {
  if (!medicationConfig || !medicationConfig.enabled) return null;
  const logs = medicationConfig.today_logs || [];
  const now = new Date();
  const nowSec = now.getHours() * 3600 + now.getMinutes() * 60 + now.getSeconds();

  let best = null;
  for (const log of logs) {
    if (log.status !== "Pending") continue;
    const m = /^(\d{1,2}):(\d{2})$/.exec(log.scheduled_time || "");
    if (!m) continue;
    const targetSec = parseInt(m[1], 10) * 3600 + parseInt(m[2], 10) * 60;
    const remaining = targetSec - nowSec;
    if (remaining < 0) continue; // 已过，跳过
    if (best === null || remaining < best.remaining) {
      best = {
        scheduledTime: log.scheduled_time,
        medicationName: log.medication_name,
        remaining,
      };
    }
  }
  return best; // { scheduledTime, remaining, medicationName } 或 null
}

// ==================== 喝水提醒 ====================

let currentWaterMode = "schedule"; // "schedule" | "interval"

async function loadWaterConfig() {
  try {
    waterConfig = await invoke("get_water_config");
    applyWaterConfigToUI();
    updateWaterStats();
  } catch (e) {
    console.error("Failed to load water config:", e);
  }
}

function applyWaterConfigToUI() {
  if (!waterConfig) return;
  
  document.getElementById("water-enabled").checked = waterConfig.enabled;
  toggleWaterConfig(waterConfig.enabled);
  
  // 设置模式
  currentWaterMode = waterConfig.schedule_enabled ? "schedule" : "interval";
  setWaterModeUI(currentWaterMode);
  
  // 通用设置
  const cupVal = waterConfig.cup_size_ml || 250;
  document.getElementById("water-cup-general").value = cupVal;
  document.getElementById("water-cup-general-value").textContent = `${cupVal}ml`;
  
  // 间隔模式设置
  document.getElementById("water-interval").value = waterConfig.interval_minutes;
  document.getElementById("water-interval-value").textContent = `${waterConfig.interval_minutes}分钟`;
  document.getElementById("water-goal").value = waterConfig.daily_goal_ml;
  document.getElementById("water-goal-value").textContent = `${waterConfig.daily_goal_ml}ml`;
  document.getElementById("water-goal-display").textContent = waterConfig.daily_goal_ml;
  document.getElementById("water-cup").value = cupVal;
  document.getElementById("water-cup-value").textContent = `${cupVal}ml`;
  document.getElementById("water-start-hour").value = waterConfig.start_hour;
  document.getElementById("water-end-hour").value = waterConfig.end_hour;
}

function setWaterMode(mode) {
  currentWaterMode = mode;
  setWaterModeUI(mode);
  saveWaterConfig();
}

function setWaterModeUI(mode) {
  const scheduleBtn = document.getElementById("mode-schedule");
  const intervalBtn = document.getElementById("mode-interval");
  const schedulePanel = document.getElementById("schedule-panel");
  const intervalPanel = document.getElementById("interval-panel");
  
  scheduleBtn.classList.toggle("active", mode === "schedule");
  intervalBtn.classList.toggle("active", mode === "interval");
  schedulePanel.classList.toggle("hidden", mode !== "schedule");
  intervalPanel.classList.toggle("hidden", mode !== "interval");
}
window.setWaterMode = setWaterMode;

function renderScheduleSlots(slots) {
  const container = document.getElementById("schedule-list");
  if (!container || !slots || slots.length === 0) {
    if (container) container.innerHTML = '<div class="schedule-empty">暂无排班数据</div>';
    return;
  }
  
  container.innerHTML = slots.map(slot => {
    const stateClass = slot.completed ? 'slot-completed' : (slot.is_current ? 'slot-current' : 'slot-pending');
    const stateIcon = slot.completed ? '✅' : (slot.is_current ? '⏰' : '⬜');
    const currentBadge = slot.is_current ? '<span class="slot-badge">当前</span>' : '';
    return `<div class="schedule-slot ${stateClass}">
      <span class="slot-icon">${slot.icon}</span>
      <div class="slot-info">
        <div class="slot-header">
          <span class="slot-label">${slot.label} ${currentBadge}</span>
          <span class="slot-amount">${slot.amount_ml}ml</span>
        </div>
        <span class="slot-time">${slot.time_range}</span>
      </div>
      <span class="slot-status">${stateIcon}</span>
    </div>`;
  }).join('');
}

async function saveWaterConfig() {
  if (!waterConfig) return;
  
  const cupSize = parseInt(document.getElementById("water-cup-general").value);
  const newConfig = {
    enabled: document.getElementById("water-enabled").checked,
    interval_minutes: parseInt(document.getElementById("water-interval").value),
    daily_goal_ml: parseInt(document.getElementById("water-goal").value),
    cup_size_ml: cupSize,
    sound_enabled: waterConfig.sound_enabled,
    start_hour: parseInt(document.getElementById("water-start-hour").value),
    end_hour: parseInt(document.getElementById("water-end-hour").value),
    stats: waterConfig.stats,
    schedule_enabled: currentWaterMode === "schedule",
  };
  
  try {
    await invoke("save_water_config", { newConfig });
    waterConfig = newConfig;
  } catch (e) {
    console.error("Failed to save water config:", e);
  }
}

async function drinkWater() {
  try {
    const status = await invoke("drink_one_cup");
    waterConfig.stats = {
      today_date: waterConfig.stats.today_date,
      total_ml: status.total_ml,
      drink_count: status.drink_count,
      last_drink_time: status.last_drink_time,
      schedule_completed: status.schedule_slots ? 
        status.schedule_slots.map(s => s.completed) : 
        (waterConfig.stats.schedule_completed || []),
    };
    updateWaterStats();
  } catch (e) {
    console.error("Failed to record drink:", e);
  }
}

async function updateWaterStats() {
  try {
    const status = await invoke("get_water_status");
    
    document.getElementById("water-total").textContent = status.total_ml;
    document.getElementById("water-total-display").textContent = status.total_ml;
    
    // 更新 header
    if (status.schedule_enabled) {
      document.getElementById("header-water-info").textContent = 
        `💧 ${status.schedule_completed_count}/${status.schedule_total_slots}`;
    } else {
      document.getElementById("header-water-info").textContent = `💧 ${status.total_ml}ml`;
    }
    
    // 进度条
    const progressFill = document.getElementById("water-progress-fill");
    if (status.schedule_enabled) {
      const percent = status.schedule_total_slots > 0 
        ? (status.schedule_completed_count / status.schedule_total_slots * 100) : 0;
      progressFill.style.width = `${percent}%`;
      document.getElementById("schedule-progress-text").textContent = 
        `${status.schedule_completed_count}/${status.schedule_total_slots} 时段`;
    } else {
      progressFill.style.width = `${status.progress_percent}%`;
      document.getElementById("water-goal-display").textContent = status.daily_goal_ml;
    }
    
    // 渲染排班列表
    if (status.schedule_enabled && status.schedule_slots) {
      renderScheduleSlots(status.schedule_slots);
    }
    
    waterNextReminder = status.next_reminder_seconds || 0;
  } catch (e) {
    console.error("Failed to get water status:", e);
  }
}

// 注：waterNextReminder 的递减与渲染已统一在 startLocalCountdown 中处理
function startWaterCountdown() {
  // 保留函数占位（兼容性调用），实际逻辑合并到 startLocalCountdown
}

async function setupWaterNotificationListener() {
  await listen("water-reminder", async (event) => {
    console.log("[water] reminder event received:", event.payload);
    const body = typeof event.payload === "string" ? event.payload : "该喝水啦～";
    try {
      // 弹居中 modal（主通道，独立于独立窗口）
      showWaterModal(body);
      console.log("[water] modal shown");
      // 刷新数据
      await updateWaterStats();
    } catch (e) {
      console.error("[water] modal render failed:", e);
    }
  });

  // 外部窗口关闭时，同步关闭内部 modal
  await listen("water-reminder-closed", () => {
    const overlay = document.getElementById("water-modal-overlay");
    if (overlay) {
      clearTimeout(overlay._timer);
      overlay.classList.remove("show");
      setTimeout(() => overlay.remove(), 300);
    }
  });

  console.log("[water] reminder listener registered");
}

/**
 * 喝水居中 modal 弹窗（主通道）
 * 总是能在用户当前可见的应用窗口中弹出
 * 配合 Rust 端独立 always_on_top 窗口，可实现"双通道"提醒
 */
function showWaterModal(body) {
  // 移除已存在的
  const old = document.getElementById("water-modal-overlay");
  if (old) old.remove();

  const overlay = document.createElement("div");
  overlay.id = "water-modal-overlay";
  overlay.className = "water-modal-overlay";

  const card = document.createElement("div");
  card.className = "water-modal";
  card.innerHTML = `
    <div class="water-modal-icon">💧</div>
    <div class="water-modal-title">喝水时间到</div>
    <div class="water-modal-msg">${escapeHtml(body)}</div>
    <div class="water-modal-actions">
      <button class="water-modal-btn water-modal-btn-primary" data-act="drink">🥤 喝了一杯</button>
      <button class="water-modal-btn" data-act="half">💧 半杯</button>
      <button class="water-modal-btn water-modal-btn-ghost" data-act="dismiss">✕ 稍后</button>
    </div>
  `;

  overlay.appendChild(card);
  document.body.appendChild(overlay);

  requestAnimationFrame(() => overlay.classList.add("show"));

  // 关闭函数
  function close() {
    overlay.classList.remove("show");
    setTimeout(() => overlay.remove(), 300);
    document.removeEventListener("keydown", escHandler);
  }
  function escHandler(e) {
    if (e.key === "Escape") close();
  }
  document.addEventListener("keydown", escHandler);

  // 点击遮罩关闭（点击卡片不关闭）
  overlay.addEventListener("click", (e) => {
    if (e.target === overlay) close();
  });

  // 按钮事件
  card.addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-act]");
    if (!btn) return;
    btn.disabled = true;
    try {
      if (btn.dataset.act === "drink") {
        const cupMl = (waterConfig && waterConfig.cup_size_ml) || 250;
        await invoke("record_water_intake", { ml: cupMl });
      } else if (btn.dataset.act === "half") {
        const cupMl = Math.round(((waterConfig && waterConfig.cup_size_ml) || 250) / 2);
        await invoke("record_water_intake", { ml: cupMl });
      }
      close();
      await updateWaterStats();
    } catch (err) {
      console.error("Water modal action failed:", err);
      btn.disabled = false;
    }
  });

  // 90 秒自动关闭
  overlay._timer = setTimeout(close, 90000);
}

/**
 * 喝水持久化 toast（最长 60 秒）
 * 支持"我喝了"快捷按钮（直接调用 recordWaterIntake）
 */
function showWaterToast(body) {
  let container = document.getElementById("water-toast-container");
  if (!container) {
    container = document.createElement("div");
    container.id = "water-toast-container";
    container.className = "water-toast-container";
    document.body.appendChild(container);
  }

  const el = document.createElement("div");
  el.className = "water-toast";
  el.innerHTML = `
    <div class="water-toast-icon">💧</div>
    <div class="water-toast-body">
      <div class="water-toast-title">💧 喝水时间到</div>
      <div class="water-toast-msg">${escapeHtml(body)}</div>
      <div class="water-toast-actions">
        <button class="water-toast-btn water-toast-btn-primary" data-act="drink">🥤 喝了一杯</button>
        <button class="water-toast-btn" data-act="half">💧 半杯</button>
        <button class="water-toast-btn water-toast-btn-ghost" data-act="dismiss">✕ 稍后</button>
      </div>
    </div>
  `;

  el.addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-act]");
    if (!btn) return;
    btn.disabled = true;
    try {
      if (btn.dataset.act === "drink") {
        const cupMl = (waterConfig && waterConfig.cup_size_ml) || 250;
        await invoke("record_water_intake", { ml: cupMl });
      } else if (btn.dataset.act === "half") {
        const cupMl = Math.round(((waterConfig && waterConfig.cup_size_ml) || 250) / 2);
        await invoke("record_water_intake", { ml: cupMl });
      }
      clearWaterToast(el);
      await updateWaterStats();
    } catch (err) {
      console.error("Water toast action failed:", err);
      btn.disabled = false;
    }
  });

  container.appendChild(el);
  requestAnimationFrame(() => el.classList.add("show"));

  const total = 90000; // 90 秒自动消失，鼠标悬停暂停
  el._timer = setTimeout(() => clearWaterToast(el), total);
  el.addEventListener("mouseenter", () => clearTimeout(el._timer));
  el.addEventListener("mouseleave", () => {
    el._timer = setTimeout(() => clearWaterToast(el), total);
  });
}

function clearWaterToast(el) {
  if (!el || !el.parentNode) return;
  clearTimeout(el._timer);
  el.classList.remove("show");
  setTimeout(() => el.parentNode && el.parentNode.removeChild(el), 300);
}

// ==================== 其他监听 ====================

async function setupNotificationListener() {
  await listen("trigger-notification", async () => {
    console.log("Notification triggered");
  });
}

async function setupFullscreenListener() {
  await listen("trigger-fullscreen", async (event) => {
    const payload = event.payload;
    console.log("Fullscreen triggered", payload);
    
    // 构建 URL 参数，包含 AI 内容
    const params = new URLSearchParams({
      duration: 30,
      theme: '#E8F4F8',
      forced: payload.forced,
      severity: payload.severity,
      eye_health: payload.eye_health,
      skip_history: btoa(JSON.stringify(payload.skip_history || [])),
      total_skipped: payload.total_skipped_seconds || 0,
    });
    
    // 添加 AI 内容（如果存在）
    if (payload.ai_title) params.set('ai_title', payload.ai_title);
    if (payload.ai_main_text) params.set('ai_main_text', payload.ai_main_text);
    if (payload.ai_sub_text) params.set('ai_sub_text', payload.ai_sub_text);
    if (payload.ai_interaction) params.set('ai_interaction', payload.ai_interaction);
    
    try {
      await invoke("show_fullscreen", { 
        forced: payload.forced, 
        severity: payload.severity, 
        eye_health: payload.eye_health,
        skip_history_json: JSON.stringify(payload.skip_history || []),
        total_skipped_seconds: payload.total_skipped_seconds || 0,
        ai_title: payload.ai_title,
        ai_main_text: payload.ai_main_text,
        ai_sub_text: payload.ai_sub_text,
        ai_interaction: payload.ai_interaction,
      });
    } catch (e) {
      console.error("Failed to show fullscreen:", e);
    }
  });
}

async function setupResumeListener() {
  await listen("system-resumed", (event) => {
    console.log("System resumed from sleep, gap:", event.payload);
  });
}

// ==================== 用药提醒 ====================

let medicationConfig = null;
let medicationNextSeconds = 0;
let editingMedicationId = "";

async function loadMedicationConfig() {
  try {
    medicationConfig = await invoke("get_medication_config");
    applyMedicationConfigToUI();
    renderMedicationToday();
    renderMedicationList();
    updateMedicationAdherence();
  } catch (e) {
    console.error("Failed to load medication config:", e);
  }
}

function applyMedicationConfigToUI() {
  if (!medicationConfig) return;

  document.getElementById("medication-enabled").checked = medicationConfig.enabled;
  toggleMedicationConfig(medicationConfig.enabled);
  document.getElementById("med-escalation").value = medicationConfig.escalation_minutes || 30;
  document.getElementById("med-escalation-value").textContent = `${medicationConfig.escalation_minutes || 30}分钟`;
  document.getElementById("med-notify").checked = medicationConfig.notifications_enabled !== false;
  document.getElementById("med-confirm").checked = medicationConfig.confirm_required !== false;
}

function toggleMedicationConfig(show) {
  document.getElementById("medication-config").classList.toggle("hidden", !show);
}

async function saveMedicationConfig() {
  if (!medicationConfig) return;
  const newCfg = {
    ...medicationConfig,
    enabled: document.getElementById("medication-enabled").checked,
    notifications_enabled: document.getElementById("med-notify").checked,
    confirm_required: document.getElementById("med-confirm").checked,
    escalation_minutes: parseInt(document.getElementById("med-escalation").value, 10),
  };
  try {
    await invoke("save_medication_config", { newConfig: newCfg });
    medicationConfig = newCfg;
    await loadMedicationConfig();
  } catch (e) {
    console.error("Failed to save medication config:", e);
  }
}

function renderMedicationToday() {
  const listEl = document.getElementById("med-today-list");
  if (!listEl) return;
  const logs = (medicationConfig && medicationConfig.today_logs) || [];
  if (logs.length === 0) {
    listEl.innerHTML = '<div class="med-empty">还没有药品，添加一个开始吧 👇</div>';
    return;
  }
  listEl.innerHTML = logs.map((log) => {
    const status = log.status || "Pending";
    const stateClass = `med-log-${status.toLowerCase()}`;
    const stateLabel = ({
      Pending: "⏰ 待服药",
      Taken: "✅ 已服用",
      Skipped: "🚫 已跳过",
      Delayed: "🕐 延迟",
      Missed: "❌ 错过",
    })[status] || status;
    let actions = "";
    if (status === "Pending") {
      actions = `
        <button class="med-btn med-btn-primary" data-act="confirm" data-id="${log.id}">✅ 已服</button>
        <button class="med-btn" data-act="snooze" data-id="${log.id}">⏱ 10分钟</button>
        <button class="med-btn" data-act="skip" data-id="${log.id}">🚫 跳过</button>
      `;
    }
    const note = log.notes ? `<span class="med-log-note">${escapeHtml(log.notes)}</span>` : "";
    return `
      <div class="med-log ${stateClass}">
        <div class="med-log-time">${escapeHtml(log.scheduled_time)}</div>
        <div class="med-log-body">
          <div class="med-log-name">${escapeHtml(log.medication_name)}</div>
          ${note}
        </div>
        <div class="med-log-status">${stateLabel}</div>
        <div class="med-log-actions">${actions}</div>
      </div>
    `;
  }).join("");
}

function renderMedicationList() {
  const listEl = document.getElementById("med-list");
  if (!listEl) return;
  const meds = (medicationConfig && medicationConfig.medications) || [];
  if (meds.length === 0) {
    listEl.innerHTML = '<div class="med-empty">尚未添加药品</div>';
    return;
  }
  listEl.innerHTML = meds.map((m) => {
    const stockTxt = m.stock_remaining != null
      ? `📦 库存 ${m.stock_remaining}${escapeHtml(m.unit || "")}`
      : "";
    return `
      <div class="med-item">
        <div class="med-item-icon" style="background:${escapeAttr(m.color || '#4CAF50')}">${escapeHtml(m.icon || "💊")}</div>
        <div class="med-item-body">
          <div class="med-item-title">
            <span>${escapeHtml(m.name)}</span>
            <span class="med-item-dose">${escapeHtml(m.dosage || "")} × ${m.quantity_per_dose || 1}${escapeHtml(m.unit || "")}</span>
          </div>
          <div class="med-item-meta">
            <span>${(m.schedule.times || []).map(t => `${pad2(t.hour)}:${pad2(t.minute)}`).join(" / ") || "未设置"}</span>
            ${stockTxt ? `<span>${stockTxt}</span>` : ""}
            <span class="med-item-status ${m.enabled ? 'on' : 'off'}">${m.enabled ? '已启用' : '已暂停'}</span>
          </div>
        </div>
        <div class="med-item-actions">
          <button class="med-btn" data-act="edit" data-id="${escapeAttr(m.id)}">编辑</button>
          <button class="med-btn" data-act="toggle" data-id="${escapeAttr(m.id)}">${m.enabled ? '停用' : '启用'}</button>
          <button class="med-btn med-btn-danger" data-act="delete" data-id="${escapeAttr(m.id)}">删除</button>
        </div>
      </div>
    `;
  }).join("");
}

function pad2(n) {
  n = parseInt(n, 10) || 0;
  return n < 10 ? `0${n}` : `${n}`;
}

async function updateMedicationAdherence() {
  try {
    const adherence = await invoke("get_medication_adherence");
    const fill = document.getElementById("med-adherence-fill");
    const val = document.getElementById("med-adherence-value");
    if (fill) fill.style.width = `${adherence}%`;
    if (val) val.textContent = `${adherence}%`;
  } catch (e) {
    console.error("Failed to get adherence:", e);
  }
}

async function onMedicationFormSubmit(e) {
  e.preventDefault();
  const id = document.getElementById("med-id").value;
  const times = [];
  for (let i = 1; i <= 3; i++) {
    const t = document.getElementById(`med-time-${i}`).value;
    const lbl = document.getElementById(`med-time-label-${i}`).value;
    if (t) {
      const [h, m] = t.split(":");
      times.push({ hour: parseInt(h, 10), minute: parseInt(m, 10), label: lbl || "" });
    }
  }
  if (times.length === 0) {
    alert("请至少设置一个服药时间");
    return;
  }
  const stockRaw = document.getElementById("med-stock").value;
  const stock = stockRaw === "" ? null : parseFloat(stockRaw);

  // 若是编辑模式，从 medicationConfig 中取出原 schedule 的不可变字段，避免数据丢失
  const original = id ? medicationConfig.medications.find((m) => m.id === id) : null;
  const originalSchedule = original ? original.schedule : null;

  const med = {
    id: id || "",
    name: document.getElementById("med-name").value.trim(),
    generic_name: original ? original.generic_name : "",
    dosage: document.getElementById("med-dosage").value.trim(),
    form: document.getElementById("med-form").value,
    unit: document.getElementById("med-unit").value.trim() || "片",
    quantity_per_dose: parseFloat(document.getElementById("med-quantity").value) || 1,
    schedule: {
      times,
      relation: document.getElementById("med-relation").value,
      // 编辑时若表单未提供 days（UI 未暴露该字段），沿用原值
      days: originalSchedule && originalSchedule.days ? originalSchedule.days : [],
      start_date:
        (originalSchedule && originalSchedule.start_date) ||
        new Date().toISOString().split("T")[0],
      end_date: originalSchedule ? originalSchedule.end_date : null,
    },
    notes: document.getElementById("med-notes").value,
    color: document.getElementById("med-color").value,
    icon: document.getElementById("med-icon").value || "💊",
    // 编辑时沿用原 enabled；添加时默认 true
    enabled: original ? original.enabled : true,
    // 编辑时若库存输入框留空，沿用原库存
    stock_remaining: stock != null ? stock : (original ? original.stock_remaining : null),
    stock_alert_threshold: parseFloat(document.getElementById("med-stock-threshold").value) || 7,
    created_at: original ? original.created_at : "",
    tags: original ? original.tags : [],
    interval_hours: 0,
  };

  console.log("[med] submitting:", { id: med.id, name: med.name, op: id ? "update" : "add" });
  try {
    if (id) {
      await invoke("update_medication", { medication: med });
    } else {
      await invoke("add_medication", { medication: med });
    }
    resetMedicationForm();
    await loadMedicationConfig();
  } catch (err) {
    console.error("[med] save failed:", err);
    alert(`保存失败: ${err}`);
  }
}

function resetMedicationForm() {
  editingMedicationId = "";
  document.getElementById("med-form-title").textContent = "➕ 添加药品";
  document.getElementById("med-form").reset();
  document.getElementById("med-id").value = "";
  document.getElementById("med-form").value = "Tablet";
  document.getElementById("med-quantity").value = "1";
  document.getElementById("med-unit").value = "片";
  document.getElementById("med-time-1").value = "08:00";
  document.getElementById("med-time-label-1").value = "早餐后";
  document.getElementById("med-time-2").value = "";
  document.getElementById("med-time-label-2").value = "";
  document.getElementById("med-time-3").value = "";
  document.getElementById("med-time-label-3").value = "";
  document.getElementById("med-color").value = "#4CAF50";
  document.getElementById("med-icon").value = "💊";
  document.getElementById("med-relation").value = "AfterMeal";
  document.getElementById("med-cancel-btn").style.display = "none";
}

function startEditingMedication(med) {
  editingMedicationId = med.id;
  document.getElementById("med-form-title").textContent = "✏️ 编辑药品";
  document.getElementById("med-id").value = med.id;
  document.getElementById("med-name").value = med.name || "";
  document.getElementById("med-form").value = med.form || "Tablet";
  document.getElementById("med-dosage").value = med.dosage || "";
  document.getElementById("med-quantity").value = med.quantity_per_dose || 1;
  document.getElementById("med-unit").value = med.unit || "片";
  document.getElementById("med-relation").value = med.schedule?.relation || "AnyTime";
  document.getElementById("med-color").value = med.color || "#4CAF50";
  document.getElementById("med-icon").value = med.icon || "💊";
  document.getElementById("med-stock").value = med.stock_remaining ?? "";
  document.getElementById("med-stock-threshold").value = med.stock_alert_threshold || 7;
  document.getElementById("med-notes").value = med.notes || "";

  const times = med.schedule?.times || [];
  for (let i = 0; i < 3; i++) {
    const t = times[i] || { hour: 0, minute: 0, label: "" };
    document.getElementById(`med-time-${i + 1}`).value =
      t.hour || t.minute ? `${pad2(t.hour)}:${pad2(t.minute)}` : "";
    document.getElementById(`med-time-label-${i + 1}`).value = t.label || "";
  }
  document.getElementById("med-cancel-btn").style.display = "inline-block";
  document.getElementById("medication-config").scrollIntoView({ behavior: "smooth" });
}

async function onMedicationAction(e) {
  const btn = e.target.closest("[data-act]");
  if (!btn) return;
  const act = btn.dataset.act;
  const id = btn.dataset.id;
  if (!id) return;
  try {
    if (act === "confirm") {
      await invoke("confirm_dose", { logId: id });
    } else if (act === "skip") {
      const reason = prompt("请输入跳过原因（可留空）：") || null;
      await invoke("skip_dose", { logId: id, reason });
    } else if (act === "snooze") {
      await invoke("snooze_dose", { logId: id, minutes: 10 });
    } else if (act === "edit") {
      const med = medicationConfig.medications.find((m) => m.id === id);
      if (med) startEditingMedication(med);
      return;
    } else if (act === "delete") {
      if (!confirm("确认删除此药品？")) return;
      await invoke("delete_medication", { medicationId: id });
    } else if (act === "toggle") {
      const med = medicationConfig.medications.find((m) => m.id === id);
      if (!med) return;
      med.enabled = !med.enabled;
      await invoke("update_medication", { medication: med });
    }
    await loadMedicationConfig();
  } catch (err) {
    alert(`操作失败: ${err}`);
  }
}

function setupMedicationListeners() {
  // 列表/今日计划操作
  document.getElementById("med-today-list").addEventListener("click", onMedicationAction);
  document.getElementById("med-list").addEventListener("click", onMedicationAction);

  // 后端事件：服药/跳过/稍后（从提醒窗口触发后通知主窗口同步）
  listen("medication-dosed", async () => {
    await loadMedicationConfig();
  });
  listen("medication-snoozed", async () => {
    await loadMedicationConfig();
  });

  // 后端事件：弹窗主窗 toast（提醒窗口是 always-on-top 持久化的，主窗这里也显示一条带操作按钮的 toast）
  listen("medication-reminder", async (event) => {
    const log = event.payload;
    if (!log) return;
    // 主窗 fallback toast（10 分钟自动消失，含操作按钮）
    showMedicationToast(buildReminderToastData(log));
    await loadMedicationConfig();
  });

  listen("medication-stock-alert", async (event) => {
    const text = event.payload || "药品库存不足";
    showMedicationToast({ kind: "stock", text });
  });

  // 外部窗口关闭时，同步关闭内部 toast
  listen("medication-reminder-closed", () => {
    const container = document.getElementById("med-toast-container");
    if (container) {
      container.querySelectorAll(".med-toast").forEach((el) => {
        clearToast(el);
      });
    }
  });
}

function buildReminderToastData(log) {
  const sev = log.severity || 0;
  const sevText = ["⏰ 准时提醒", "💊 该吃药", "⚠️ 稍延迟", "🔴 已超时"][Math.min(sev, 3)];
  return {
    kind: "reminder",
    logId: log.id,
    title: `${sevText} · ${log.medication_name}`,
    body: `计划时间 ${log.scheduled_time}`,
  };
}

/**
 * 持久化 toast（最长 10 分钟），支持操作按钮
 * @param {string|object} payload - 字符串则按旧版显示；对象则按新版带操作按钮
 */
function showMedicationToast(payload, durationMs = 60000) {
  // 兼容旧调用
  if (typeof payload === "string") {
    payload = { kind: "text", text: payload };
  }

  let container = document.getElementById("med-toast-container");
  if (!container) {
    container = document.createElement("div");
    container.id = "med-toast-container";
    container.className = "med-toast-container";
    document.body.appendChild(container);
  }

  const el = document.createElement("div");
  el.className = `med-toast med-toast-${payload.kind || "text"}`;

  let html = "";
  if (payload.kind === "reminder") {
    html = `
      <div class="med-toast-title">${escapeHtml(payload.title || "💊 用药提醒")}</div>
      <div class="med-toast-body">${escapeHtml(payload.body || "")}</div>
      <div class="med-toast-actions">
        <button class="med-toast-btn med-toast-btn-primary" data-act="confirm" data-id="${escapeAttr(payload.logId)}">✅ 已服</button>
        <button class="med-toast-btn" data-act="snooze" data-id="${escapeAttr(payload.logId)}">⏱ 10分钟</button>
        <button class="med-toast-btn" data-act="skip" data-id="${escapeAttr(payload.logId)}">🚫 跳过</button>
        <button class="med-toast-btn med-toast-btn-ghost" data-act="dismiss">✕</button>
      </div>
    `;
  } else {
    html = `<div class="med-toast-body">${escapeHtml(payload.text || "")}</div>`;
  }
  el.innerHTML = html;

  // 绑定按钮
  el.addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-act]");
    if (!btn) return;
    const act = btn.dataset.act;
    if (act === "dismiss") {
      clearToast(el);
      return;
    }
    try {
      btn.disabled = true;
      if (act === "confirm") {
        await invoke("confirm_dose", { logId: btn.dataset.id });
      } else if (act === "skip") {
        await invoke("skip_dose", { logId: btn.dataset.id, reason: "通过主窗 toast 跳过" });
      } else if (act === "snooze") {
        await invoke("snooze_dose", { logId: btn.dataset.id, minutes: 10 });
      }
      clearToast(el);
      await loadMedicationConfig();
    } catch (err) {
      console.error("Toast action failed:", err);
      btn.disabled = false;
    }
  });

  container.appendChild(el);
  // 触发动画
  requestAnimationFrame(() => el.classList.add("show"));

  el._timer = setTimeout(() => clearToast(el), durationMs);
  el.addEventListener("mouseenter", () => clearTimeout(el._timer));
  el.addEventListener("mouseleave", () => {
    el._timer = setTimeout(() => clearToast(el), durationMs);
  });
}

function clearToast(el) {
  if (!el || !el.parentNode) return;
  clearTimeout(el._timer);
  el.classList.remove("show");
  setTimeout(() => el.parentNode && el.parentNode.removeChild(el), 300);
}

function startMedicationCountdown() {
  setInterval(async () => {
    if (medicationConfig && medicationConfig.enabled) {
      // 仅在有 pending 时计算下次时间
      const logs = medicationConfig.today_logs || [];
      const next = logs
        .filter((l) => l.status === "Pending")
        .map((l) => l.scheduled_time)
        .sort()[0];
      const nextEl = document.getElementById("med-next-text");
      if (next) {
        if (nextEl) nextEl.textContent = next;
      } else if (nextEl) {
        nextEl.textContent = "今日计划已完成 🎉";
      }
    }
  }, 30000);
}

// 全局函数：折叠区块
function toggleSection(id) {
  const content = document.getElementById(id);
  if (!content) return;
  const header = content.previousElementSibling;
  content.classList.toggle("expanded");
  header.classList.toggle("expanded");
}

// 使用事件委托统一处理折叠
document.addEventListener("DOMContentLoaded", () => {
  document.addEventListener("click", (e) => {
    const header = e.target.closest(".collapsible-header");
    if (header) {
      const content = header.nextElementSibling;
      if (content && content.classList.contains("collapsible-content")) {
        content.classList.toggle("expanded");
        header.classList.toggle("expanded");
      }
    }
  });
});
