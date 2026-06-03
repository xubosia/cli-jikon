<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  // ---------- Props（保留原搜索/命令组件） ----------
  export let browserHistory = [];
  export let commandHistory = [];
  export let onHistoryUpdate = (mode, newHistory) => {};
  export let onSwitchMode = () => {};
  export let alwaysOnTop = false;
  export let onExternalOpen = () => {};

  // ---------- 内部状态 ----------
  let engines = [];
  let currentEngineIdx = 0;
  let showEnginePicker = false;
  let shortcutMode = false;

  let inputValue = '';
  let mode = 'input';          // 'input' | 'browser' | 'command'
  let historyIndex = 0;
  let inputEl;

  // 输入法预测相关
  const PREDICT_URL = 'http://127.0.0.1:5001/predict';
  const API_BASE = 'http://127.0.0.1:5001';          // 便于调用模型相关接口
  const appWindow = getCurrentWindow();
  let currentCandidates = [];
  let isFetching = false;
  let debounceTimer = null;
  let currentK = 7;

  // 模型切换相关
  let availableModels = [];       // 从后端获取的模型列表
  let showModelPicker = false;    // 模型选择器是否显示
  let currentModelName = '';      // 当前加载的模型名

  // 根据模式动态获取历史列表（与原来一致）
  $: currentHistory = mode === 'command' ? commandHistory : browserHistory;

  // 当历史变化时，索引指向末尾（与原来一致）
  $: if (currentHistory.length > 0) {
    historyIndex = currentHistory.length;
  } else {
    historyIndex = 0;
  }

  // 自动预测：当 inputValue 变化且在输入模式下时触发
  $: if (mode === 'input' && inputValue !== '') {
    triggerPredict(inputValue);
  }

  // ========== 模型相关函数 ==========
  async function fetchAvailableModels() {
    try {
      const res = await fetch(`${API_BASE}/list_models`);
      if (res.ok) {
        const data = await res.json();
        availableModels = data.models || [];
        // 如果当前没有选定模型，默认选中第一个
        if (!currentModelName && availableModels.length > 0) {
          currentModelName = availableModels[0];
        }
      }
    } catch (e) {
      console.error('获取模型列表失败', e);
    }
  }

  async function switchModel(modelName) {
    try {
      const res = await fetch(`${API_BASE}/set_model`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model_name: modelName })
      });
      const data = await res.json();
      if (data.status === 'ok') {
        currentModelName = modelName;
        // 切换后清空候选和输入
        inputValue = '';
        renderCandidates([]);
        if (inputEl) inputEl.focus();
      } else {
        alert('切换失败: ' + (data.error || '未知错误'));
      }
    } catch (e) {
      console.error(e);
      alert('网络错误，无法切换模型');
    }
  }

  // ---------- 生命周期 ----------
  onMount(async () => {
    try {
      engines = await invoke('get_search_engines');
    } catch {
      engines = [];
    }
    await fetchAvailableModels();   // 获取可用模型列表
    if (inputEl) inputEl.focus();
  });

  // ---------- 辅助函数 ----------
  function addToHistory(item) {
    const newHistory = currentHistory.filter(h => h !== item);
    newHistory.unshift(item);
    const trimmed = newHistory.slice(0, 50);
    onHistoryUpdate(mode, trimmed);
    return trimmed.length;
  }

  // 搜索/命令执行（保留原有逻辑）
  async function executeInput(raw) {
    if (mode === 'browser') {
      let url;
      if (raw.startsWith('http://') || raw.startsWith('https://')) {
        url = raw;
      } else if (raw.startsWith('www.')) {
        url = 'http://' + raw;
      } else if (raw.includes('.') && !raw.includes(' ') && !raw.includes('\t')) {
        url = 'http://' + raw;
      } else {
        const engine = engines[currentEngineIdx] || {
          url_template: 'https://www.google.com/search?q={q}'
        };
        url = engine.url_template.replace('{q}', encodeURIComponent(raw));
      }
      await openUrl(url);
      onExternalOpen();
    } else if (mode === 'command') {
      await invoke('execute_command', { cmd: raw });
      onExternalOpen();
    }
  }

  // Enter 键处理（根据模式分支）
  async function onEnter() {
    const raw = inputValue.trim();
    if (!raw) return;
    if (mode === 'input') {
      await handleInputModeEnter();
    } else {
      try {
        await executeInput(raw);
      } catch (e) {
        alert(`执行失败: ${e}`);
        return;
      }
      const newLen = addToHistory(raw);
      historyIndex = newLen;
      inputValue = '';
    }
  }

  // 输入法 Enter 发送并隐藏窗口
  async function handleInputModeEnter() {
    const raw = inputValue.trim();
    if (!raw) return;
    inputValue = '';
    renderCandidates([]);
    try {
      await invoke('simulate_input_and_hide', { text: raw });
    } catch (e) {
      console.error(e);
    }
  }

  // 从历史填充（保留原有）
  function fillFromHistory(item, idx) {
    inputValue = item;
    historyIndex = idx;
    if (inputEl) inputEl.focus();
    if (mode === 'input') triggerPredict(item);
  }

  // 追加候选词
  function appendWord(word) {
    inputValue += word;
    triggerPredict(inputValue);
    inputEl?.focus();
  }

  // ---------- 输入法预测 ----------
  async function fetchPredictions(text) {
    if (!text || text.trim() === '' || mode !== 'input') {
      renderCandidates([]);
      return;
    }
    if (isFetching) return;
    isFetching = true;
    try {
      const res = await fetch(PREDICT_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text, k: currentK })
      });
      if (!res.ok) throw new Error('predict failed');
      const data = await res.json();
      renderCandidates(data.candidates || []);
    } catch (err) {
      console.error(err);
      renderCandidates([]);
    } finally {
      isFetching = false;
    }
  }

  function triggerPredict(text) {
    if (mode !== 'input') return;
    clearTimeout(debounceTimer);
    if (!text || text.trim() === '') {
      renderCandidates([]);
      return;
    }
    debounceTimer = setTimeout(() => fetchPredictions(text), 280);
  }

  function renderCandidates(candidates) {
    currentCandidates = candidates || [];
  }

  // ---------- 模式切换 ----------
  function switchMode(targetMode) {
    mode = targetMode;
    shortcutMode = false;
    showEnginePicker = false;
    showModelPicker = false;   // 关闭模型选择器
    inputValue = '';
    historyIndex = currentHistory.length;
    renderCandidates([]);
    inputEl?.focus();
  }

  function selectEngine(idx) {
    currentEngineIdx = idx;
    showEnginePicker = false;
    if (inputEl) inputEl.focus();
  }

  // ---------- 键盘处理（融合原逻辑 + 新快捷键）----------
  function handleKey(e) {
    // 引擎选择器
    if (showEnginePicker) {
      if (e.key === 'Escape') { showEnginePicker = false; e.preventDefault(); return; }
      if (/^[0-9]$/.test(e.key)) {
        const idx = e.key === '0' ? 9 : Number(e.key) - 1;
        if (idx < engines.length) selectEngine(idx);
        e.preventDefault();
        return;
      }
      return;
    }

    // 模型选择器
    if (showModelPicker) {
      if (e.key === 'Escape') { showModelPicker = false; e.preventDefault(); return; }
      // 数字键选择模型（如果未来想支持，可加，但当前列表较长不宜用数字）
      return;
    }

    // 快捷模式（搜索/命令）
    if (shortcutMode && mode !== 'input') {
      if (e.key === 'F' && e.shiftKey) { shortcutMode = false; e.preventDefault(); return; }
      if (/^[0-9]$/.test(e.key)) {
        const idx = e.key === '0' ? 9 : Number(e.key) - 1;
        if (idx < currentHistory.length) {
          executeInput(currentHistory[idx]);
          inputValue = '';
        }
        e.preventDefault();
        return;
      }
      return;
    }

    // Shift+R 打开引擎选择器（搜索/命令模式）
    if (e.key === 'R' && e.shiftKey && mode !== 'input') {
      showEnginePicker = true;
      e.preventDefault();
      return;
    }

    // Shift+F 快捷模式
    if (e.key === 'F' && e.shiftKey && mode !== 'input') {
      shortcutMode = true;
      e.preventDefault();
      return;
    }

    // 模式切换 (Shift+A: input ↔ browser, Shift+S: input ↔ command)
    if (e.key === 'A' && e.shiftKey) {
      e.preventDefault();
      if (mode === 'input') switchMode('browser');
      else if (mode === 'browser') switchMode('input');
      else switchMode('input');  // 从 command 切回 input
      return;
    }
    if (e.key === 'S' && e.shiftKey) {
      e.preventDefault();
      if (mode === 'command') switchMode('input');
      else switchMode('command');
      return;
    }

    // 新增：Shift+C 打开模型选择器（仅在输入模式或任意模式？我们限制在输入模式）
    if (e.key === 'C' && e.shiftKey && mode === 'input') {
      showModelPicker = true;
      e.preventDefault();
      return;
    }

    // Tab 键（保留原行为）
    if (e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      onSwitchMode();
      return;
    }

    // Enter
if (e.key === 'Enter') {
  if (e.shiftKey) {
    // Shift+Enter：发送消息
    e.preventDefault();
    onEnter();
    return;
  }
  // Enter：允许默认换行，不做任何阻止
  return;
}

   

    // 输入法数字键上屏
    if (mode === 'input' && /^[1-9]$/.test(e.key)) {
      const idx = parseInt(e.key) - 1;
      if (currentCandidates.length > idx) {
        e.preventDefault();
        appendWord(currentCandidates[idx].word);
      }
    }
  }
</script>

<!-- ================= 模板：融合原样式 + 输入法候选 UI ================= -->
<div class="input-box-container">
  <!-- 模式提示（复用原 mode-hint 结构） -->
  <div class="mode-hint">
    {#if shortcutMode}
      ⚡ 快捷模式：1～0 直接执行前十条历史
      <span class="hint">Shift+F 退出</span>
    {:else}
      {mode === 'input' ? '📝 输入法模式' : mode === 'browser' ? '🌐 浏览器搜索/网址模式' : '💻 命令行模式'}
      <span class="hint">
        Shift+A {mode === 'input' ? '搜索' : '输入法'}  【Shift + |S 命令行 | R 搜索源 | F 快捷模式 | C 模型 | ↑↓ 历史】
      </span>
    {/if}
    <!-- 显示当前模型 -->
    {#if currentModelName}
      <span style="margin-left: auto; font-size:0.8rem; background:#0a2a35; padding:2px 12px; border-radius:20px; color:#2dd4bf;">
        🧠 {currentModelName}
      </span>
    {/if}
  </div>

  <!-- 搜索引擎选择器（保留） -->
  {#if showEnginePicker}
    <div class="engine-picker-overlay" on:click={() => showEnginePicker = false}>
      <div class="engine-picker" on:click|stopPropagation>
        <h3>选择搜索引擎</h3>
        <ul>
          {#each engines as engine, i}
            <li on:click={() => selectEngine(i)}>
              <span class="key-hint">{i + 1 === 10 ? 0 : i + 1}</span>
              {engine.name}
            </li>
          {/each}
        </ul>
        <p class="esc-hint">按 Esc 关闭</p>
      </div>
    </div>
  {/if}

  <!-- 模型选择器（新增） -->
  {#if showModelPicker}
    <div class="engine-picker-overlay" on:click={() => showModelPicker = false}>
      <div class="engine-picker" on:click|stopPropagation>
        <h3>🧠 选择语言模型</h3>
        <ul>
          {#each availableModels as modelName}
            <li on:click={() => { switchModel(modelName); showModelPicker = false; }}>
              {modelName}
              {#if modelName === currentModelName}
                <span style="margin-left: auto; color:#2dd4bf;">✓ 当前</span>
              {/if}
            </li>
          {/each}
        </ul>
        <p class="esc-hint">按 Esc 关闭</p>
      </div>
    </div>
  {/if}

  <!-- 输入框组 -->
  <div class="input-group">
    <textarea
      bind:this={inputEl}
      class="cyber-input"
      rows="2"
      bind:value={inputValue}
      on:keydown={handleKey}
      placeholder={mode === 'input' ? '输入文字开始续写…' : mode === 'browser' ? '输入网址或搜索内容…' : '输入终端命令…'}
      autofocus
    ></textarea>
    <button class="send-btn" on:click={onEnter}>↵ 发送</button>
  </div>

  <!-- 候选栏（输入法专属） -->
  {#if mode === 'input'}
    <div class="crystal-bar">
      <span class="crystal-label">⤦ 神经候选池</span>
      {#if currentCandidates.length === 0}
        <span class="empty-hint">输入文本后自动生成预测</span>
      {:else}
        {#each currentCandidates as cand, idx}
          <div class="candidate-tile" on:click={() => appendWord(cand.word)}>
            <span class="candidate-num">{idx + 1}</span>
            <span>{cand.word}</span>
            <span class="prob">{Math.round(cand.prob * 100)}%</span>
          </div>
        {/each}
        <span class="cand-count">⚡ {currentCandidates.length}个候选 (k={currentK}) · 数字键直达</span>
      {/if}
    </div>

    <!-- K 值滑动条 -->
    <div class="k-control">
      <span>🎛️ 候选数量 (K)</span>
      <input type="range" class="k-slider" min="1" max="12" bind:value={currentK} />
      <span class="k-value">{currentK}</span>
    </div>
  {/if}

  <!-- 历史记录列表（搜索/命令模式） -->
  {#if mode !== 'input' && currentHistory.length > 0}
    <ul class="history-dropdown">
      {#each currentHistory as item, i}
        <li
          class:active={i === historyIndex}
          on:click={() => fillFromHistory(item, i)}
        >
          {item}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  /* ==================== 全局基础样式 ==================== */
  :global(body) {
    margin: 0;
    padding: 0;
    background: radial-gradient(circle at 20% 30%, #0a0f1a, #03060c);
    font-family: 'Inter', 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', system-ui;
  }
  
  /* ==================== 输入框外层容器 ==================== */
  .input-box-container {
    padding: 1vw;
    max-width: 90vw;
    margin: 0 auto;
  }
  
  /* ==================== 模式提示标签 ==================== */
  .mode-hint {
    font-size: clamp(0.8rem, 1.2vw, 1rem);
    font-weight: 800;
    color: #ffffff;
    background: rgba(0, 0, 0, 0.65);
    padding: 0.5vh 1.5vw;
    border-radius: 3vw;
    text-shadow: 0 0.1vh 0.8vh rgba(0,0,0,0.5);
    backdrop-filter: blur(8px);
    display: inline-flex;
    align-items: center;
    gap: 0.6vw;
    margin-bottom: 1vh;
    white-space: nowrap;
  }
  
  .mode-hint .hint {
    font-size: clamp(0.7rem, 1vw, 0.9rem);
    font-weight: 700;
    color: #ffd966;
    background: rgba(0,0,0,0.4);
    padding: 0.3vh 1vw;
    border-radius: 1.5vw;
    margin-left: 0.6vw;
  }
  
  /* ==================== 引擎选择器遮罩层 ==================== */
  .engine-picker-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0,0,0,0.5);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1001;
  }
  
  /* ==================== 引擎选择器面板 ==================== */
  .engine-picker {
    background: white;
    padding: 1.5vw;
    border-radius: 1vw;
    min-width: 280px;
    max-width: 90%;
  }
  
  .engine-picker h3 {
    margin: 0 0 1vh;
    font-size: clamp(0.9rem, 1.3vw, 1rem);
  }
  
  .engine-picker ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  
  .engine-picker li {
    display: flex;
    align-items: center;
    gap: 0.6vw;
    padding: 0.6vh 0;
    cursor: pointer;
    border-bottom: 1px solid #eee;
    font-size: clamp(0.8rem, 1.1vw, 0.95rem);
  }
  
  .engine-picker li:hover {
    background: #eef2ff;
  }
  
  /* ==================== 快捷键提示键盘图标 ==================== */
  .key-hint {
    font-weight: bold;
    color: #4a90d9;
    background: #eef2ff;
    width: 2vw;
    height: 2vw;
    min-width: 1.5rem;
    min-height: 1.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 0.5vw;
    font-size: clamp(0.65rem, 0.9vw, 0.8rem);
  }
  
  /* ==================== ESC退出提示 ==================== */
  .esc-hint {
    text-align: right;
    color: #888;
    font-size: clamp(0.7rem, 0.9vw, 0.8rem);
    margin: 0.6vh 0 0 0;
  }
  
  /* ==================== 输入框与发送按钮组合 ==================== */
  .input-group {
    display: flex;
    gap: 1.5vw;
    align-items: flex-end;
    margin-bottom: 2vh;
  }
  
  /* ==================== 核心：科幻风输入框样式 ==================== */
  .cyber-input {
    flex: 1;
    background: #07131e;
    border: 0.2vw solid #2c5f6e;
    border-radius: 5vw;
    padding: 1.8vh 2.5vw;
    font-size: clamp(0.8rem, 1.3vw, 1rem);
    font-family: inherit;
    color: #e2f0ff;
    outline: none;
    transition: all 0.2s;
    font-weight: 500;
    height: 30vh;
    line-height: 1.4;
  }
  
  .cyber-input:focus {
    border-color: #2dd4bf;
    box-shadow: 0 0 0 0.3vw rgba(45,212,191,0.2);
    background: #0a1924;
  }
  
  .cyber-input::placeholder {
    color: #2e6f7a;
  }
  
  /* ==================== 发送按钮样式 ==================== */
  .send-btn {
    background: linear-gradient(145deg, #0e2a32, #05212b);
    border: 0.15vw solid #2dd4bf;
    border-radius: 5vw;
    padding: 1.5vh 3vw;
    font-weight: 600;
    font-size: clamp(0.75rem, 1.1vw, 0.85rem);
    color: #b1f5f0;
    cursor: pointer;
    transition: 0.2s;
    backdrop-filter: blur(8px);
    white-space: nowrap;
  }
  
  .send-btn:hover {
    background: #1a4a55;
    border-color: #7effe0;
    color: white;
    box-shadow: 0 0 1vw #2dd4bf60;
  }
  
  /* ==================== 候选词条容器（水晶栏）- 已缩小版 ==================== */
  .crystal-bar {
    background: rgba(4,16,24,0.8);
    border-radius: 6vw;
    padding: 0.8vh 1.5vw;        /* 👈 缩小：原来是 1.2vh 2vw */
    margin-bottom: 1.5vh;        /* 👈 缩小：原来是 2vh */
    display: flex;
    flex-wrap: wrap;
    gap: 0.8vw;                  /* 👈 缩小：原来是 1.2vw */
    align-items: center;
    border: 0.15vw solid rgba(0,255,200,0.35);
    box-shadow: 0 0.5vh 1.5vh rgba(0,0,0,0.3);
    min-height: 5vh;             /* 👈 缩小：原来是 8vh */
  }
  
  /* ==================== 水晶栏标签 - 已缩小版 ==================== */
  .crystal-label {
    font-size: clamp(0.5rem, 0.7vw, 0.6rem);   /* 👈 缩小 */
    font-weight: 600;
    letter-spacing: 0.1vw;
    color: #2dd4bf;
    background: rgba(0,20,25,0.7);
    padding: 0.3vh 0.8vw;                       /* 👈 缩小：原来是 0.5vh 1.2vw */
    border-radius: 4vw;
    border: 0.05vw solid #2dd4bf80;
  }
  
  /* ==================== 单个候选词块 - 已缩小版 ==================== */
  .candidate-tile {
    background: #0b1722;
    border: 0.15vw solid #2dd4bf60;
    border-radius: 5vw;
    padding: 0.4vh 1.5vw;                       /* 👈 缩小：原来是 0.8vh 2vw */
    font-size: clamp(0.65rem, 0.9vw, 0.8rem);   /* 👈 缩小 */
    font-weight: 500;
    color: #bcf0f0;
    cursor: pointer;
    transition: all 0.15s ease;
    display: inline-flex;
    align-items: center;
    gap: 0.6vw;                                 /* 👈 缩小：原来是 1vw */
    backdrop-filter: blur(4px);
  }
  
  .candidate-tile:hover {
    background: #0f2a32;
    border-color: #2dd4bf;
    color: white;
    transform: translateY(-0.3vh);
    box-shadow: 0 0.6vh 1.5vh rgba(0,200,200,0.2);
  }
  
  /* ==================== 候选词序号标签 - 已缩小版 ==================== */
  .candidate-num {
    background: #1a3a44;
    border-radius: 3vw;
    padding: 0.2vh 0.6vw;                       /* 👈 缩小：原来是 0.3vh 1vw */
    font-size: clamp(0.5rem, 0.65vw, 0.6rem);   /* 👈 缩小 */
    font-weight: bold;
    color: #7effe0;
  }
  
  /* ==================== 概率显示文字 ==================== */
  .prob {
    font-size: clamp(0.5rem, 0.65vw, 0.6rem);   /* 👈 缩小 */
    opacity: 0.65;
  }
  
  /* ==================== 候选词数量统计 ==================== */
  .cand-count {
    font-size: clamp(0.45rem, 0.6vw, 0.55rem);  /* 👈 缩小 */
    margin-left: auto;
    color: #2c9a8a;
  }
  
  /* ==================== 空白提示文字 ==================== */
  .empty-hint {
    color: #4f8b9a;
    font-size: clamp(0.6rem, 0.75vw, 0.7rem);   /* 👈 缩小 */
    margin-left: 0.6vw;
  }
  
  /* ==================== K值调节控件容器 ==================== */
  .k-control {
    display: flex;
    align-items: center;
    gap: 1.5vw;
    margin-bottom: 2.5vh;
    padding: 0.8vh 1.5vw;
    background: rgba(0,20,28,0.6);
    border-radius: 6vw;
    border: 0.05vw solid #2dd4bf40;
    backdrop-filter: blur(4px);
  }
  
  /* ==================== K值标签文字 ==================== */
  .k-control span:first-child {
    color: #5bc0ce;
    font-size: clamp(0.65rem, 0.85vw, 0.75rem);
    font-weight: 500;
    white-space: nowrap;
  }
  
  /* ==================== K值滑块轨道 ==================== */
  .k-slider {
    flex: 1;
    height: 0.5vh;
    -webkit-appearance: none;
    background: #1e4a55;
    border-radius: 0.5vh;
    outline: none;
  }
  
  /* ==================== K值滑块按钮（Webkit内核） ==================== */
  .k-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 1.8vw;
    height: 1.8vw;
    min-width: 12px;
    min-height: 12px;
    background: #2dd4bf;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 0 0.8vw #0ff;
    border: none;
  }
  
  /* ==================== K值数值显示 ==================== */
  .k-value {
    color: #2dd4bf;
    font-weight: bold;
    font-size: clamp(0.75rem, 1vw, 0.9rem);
    background: #0b1a22;
    padding: 0.3vh 1.2vw;
    border-radius: 3vw;
    min-width: 5vw;
    text-align: center;
    font-family: monospace;
  }
  
  /* ==================== 历史记录下拉列表 ==================== */
  .history-dropdown {
    list-style: none;
    margin: 0.6vh 0 0 0;
    padding: 0;
    border: 0.15vw solid #e0e0e0;
    border-radius: 0.8vw;
    max-height: 25vh;
    overflow-y: auto;
    background: white;
  }
  
  .history-dropdown li {
    padding: 0.6vh 1.2vw;
    cursor: pointer;
    border-bottom: 0.1vw solid #f0f0f0;
    font-size: clamp(0.8rem, 1.1vw, 0.95rem);
  }
  
  .history-dropdown li:hover,
  .history-dropdown li.active {
    background: #eef2ff;
  }
</style>