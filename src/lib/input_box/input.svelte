<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';

  // ---------- Props ----------
  export let browserHistory = [];
  export let commandHistory = [];
  export let onHistoryUpdate = (mode, newHistory) => {};
  export let onSwitchMode = () => {};   // 替代事件派发
  export let alwaysOnTop = false;
  export let onExternalOpen = () => {};

  // ---------- 内部状态 ----------
  let engines = [];               // 搜索引擎列表
  let currentEngineIdx = 0;      // 当前使用的引擎索引
  let showEnginePicker = false;  // 引擎选择器显示
  let shortcutMode = false;      // 快捷模式开关

  let inputValue = '';
  let mode = 'browser';          // 'browser' 或 'command'
  let historyIndex = 0;
  let inputEl;

  // 根据模式动态获取当前历史列表
  $: currentHistory = mode === 'browser' ? browserHistory : commandHistory;

  // 自动同步历史索引：当历史列表变化时，索引指向末尾（新输入）
  $: if (currentHistory.length > 0) {
    historyIndex = currentHistory.length;
  } else {
    historyIndex = 0;
  }

  // ---------- 生命周期 ----------
  onMount(async () => {
    try {
      engines = await invoke('get_search_engines');
    } catch {
      engines = [];
    }
    if (inputEl) inputEl.focus();
  });

  // ---------- 通用执行函数 ----------
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
    onExternalOpen();   // ← 打开浏览器后，尝试将焦点拉回应用
  } else {
    await invoke('execute_command', { cmd: raw });
    onExternalOpen();   // ← 执行终端命令后也拉回焦点
  }
}

  // 将输入添加到历史记录
  function addToHistory(item) {
    const newHistory = currentHistory.filter(h => h !== item);
    newHistory.unshift(item);
    const trimmed = newHistory.slice(0, 50);
    onHistoryUpdate(mode, trimmed);
    return trimmed.length;
  }

  // ---------- 主要操作 ----------
  async function onEnter() {
    if (!inputValue.trim()) return;
    try {
      await executeInput(inputValue.trim());
    } catch (e) {
      alert(`执行失败: ${e}`);
      return;
    }
    const newLen = addToHistory(inputValue);
    historyIndex = newLen;      // 指向末尾
    inputValue = '';
  }

  async function runHistoryItem(index) {
    const item = currentHistory[index];
    if (!item) return;
    try {
      await executeInput(item);
    } catch (e) {
      alert(`执行失败: ${e}`);
      return;
    }
    const newLen = addToHistory(item);
    historyIndex = newLen;
    inputValue = '';
  }

  function toggleMode() {
    mode = mode === 'browser' ? 'command' : 'browser';
    if (inputEl) inputEl.focus();
  }

  // 点击历史项上屏（不执行）
  function fillFromHistory(item, index) {
    inputValue = item;
    historyIndex = index;
    if (inputEl) inputEl.focus();
  }

  // ---------- 键盘处理 ----------
  function handleKey(e) {
    // 搜索引擎选择器模式
    if (showEnginePicker) {
      if (e.key === 'Escape') {
        showEnginePicker = false;
        e.preventDefault();
        return;
      }
      if (/^[0-9]$/.test(e.key)) {
        const idx = e.key === '0' ? 9 : Number(e.key) - 1;
        if (idx < engines.length) {
          currentEngineIdx = idx;
          showEnginePicker = false;
          if (inputEl) inputEl.focus();
        }
        e.preventDefault();
        return;
      }
      return;
    }

    // Shift+R 打开引擎选择器
    if (e.key === 'R' && e.shiftKey) {
      showEnginePicker = true;
      e.preventDefault();
      return;
    }

    // 快捷模式
    if (shortcutMode) {
      if (e.key === 'F' && e.shiftKey) {
        shortcutMode = false;
        e.preventDefault();
        return;
      }
      if (/^[0-9]$/.test(e.key)) {
        const idx = e.key === '0' ? 9 : Number(e.key) - 1;
        runHistoryItem(idx);
        e.preventDefault();
        return;
      }
      return;
    }

    // 普通模式下进入快捷模式
    if (e.key === 'F' && e.shiftKey) {
      shortcutMode = true;
      e.preventDefault();
      return;
    }

    // 普通模式按键
    if (e.key === 'Enter') {
      onEnter();
      e.preventDefault();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (currentHistory.length > 0) {
        if (historyIndex > 0) historyIndex--;
        inputValue = currentHistory[historyIndex] || '';
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (historyIndex < currentHistory.length - 1) {
        historyIndex++;
        inputValue = currentHistory[historyIndex] || '';
      } else {
        historyIndex = currentHistory.length;
        inputValue = '';
      }
    } else if (e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      onSwitchMode();   // 调用父组件回调
    } else if (e.key === 'A' && e.shiftKey) {
      toggleMode();
      e.preventDefault();
    }
  }
</script>

<div class="input-box-container">
  <div class="mode-hint">
    {#if shortcutMode}
      ⚡ 快捷模式：1～0 直接执行前十条历史
      <span class="hint">Shift+F 退出</span>
    {:else}
      {#if mode === 'browser'}
        🌐 浏览器搜索/网址模式
      {:else}
        💻 命令行模式
      {/if}
      <span class="hint">Shift+A 切换模式 | Shift+R 搜索源 | Shift+F 快捷模式 | Tab 返回网格 | ↑↓ 历史</span>
    {/if}
  </div>

  {#if showEnginePicker}
    <div class="engine-picker-overlay" on:click={() => showEnginePicker = false}>
      <div class="engine-picker" on:click|stopPropagation>
        <h3>选择搜索引擎</h3>
        <ul>
          {#each engines as engine, i}
            <li on:click={() => { currentEngineIdx = i; showEnginePicker = false; if (inputEl) inputEl.focus(); }}>
              <span class="key-hint">{i + 1 === 10 ? 0 : i + 1}</span>
              {engine.name}
            </li>
          {/each}
        </ul>
        <p class="esc-hint">按 Esc 关闭</p>
      </div>
    </div>
  {/if}

  <div class="input-wrapper">
    <input
      bind:this={inputEl}
      type="text"
      bind:value={inputValue}
      on:keydown={handleKey}
      placeholder={mode === 'browser' ? '输入网址或搜索内容…' : '输入终端命令…'}
      autofocus
    />
  </div>

  {#if currentHistory.length > 0}
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

.engine-picker-overlay {
    position: fixed; top: 0; left: 0; width: 100%; height: 100%;
    background: rgba(0,0,0,0.5); display: flex;
    justify-content: center; align-items: center; z-index: 1001;
}
.engine-picker {
    background: white; padding: 1rem; border-radius: 8px;
    min-width: 280px; max-width: 90%;
}
.engine-picker h3 { margin: 0 0 0.8rem; font-size: 1rem; }
.engine-picker ul { list-style: none; padding: 0; margin: 0; }
.engine-picker li {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem; cursor: pointer; border-bottom: 1px solid #eee;
}
.engine-picker li:hover { background: #eef2ff; }
.key-hint {
    font-weight: bold; color: #4a90d9; background: #eef2ff;
    width: 1.5rem; height: 1.5rem; display: flex; align-items: center;
    justify-content: center; border-radius: 4px; font-size: 0.8rem;
}
.esc-hint { text-align: right; color: #888; font-size: 0.8rem; margin: 0.5rem 0 0 0; }
  .input-box-container {
    padding: 1rem;
    max-width: 700px;
    margin: 0 auto;
  }
.mode-hint {
  font-size: 1rem;               /* 大字号 */
  font-weight: 800;                /* 最粗 */
  color: #ffffff;                  /* 纯白 */
  background: rgba(0, 0, 0, 0.65); /* 深色半透明底 */
  padding: 0.4rem 1.2rem;
  border-radius: 2rem;
  text-shadow: 0 1px 6px rgba(0,0,0,0.5);
  backdrop-filter: blur(8px);
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.8rem;
  white-space: nowrap;
}

.mode-hint .hint {
  font-size: 0.9rem;
  font-weight: 700;
  color: #ffd966;                   /* 柔和的金色 */
  background: rgba(0,0,0,0.4);
  padding: 0.2rem 0.8rem;
  border-radius: 1rem;
  margin-left: 0.5rem;
}

  .input-wrapper input {
    width: 100%;
    padding: 0.6rem 1rem;
    font-size: 1.1rem;
    border: 1px solid #ccc;
    border-radius: 8px;
    outline: none;
    transition: border 0.2s;
  }
  .input-wrapper input:focus {
    border-color: #4a90d9;
    box-shadow: 0 0 0 2px rgba(74,144,217,0.2);
  }
  .history-dropdown {
    list-style: none;
    margin: 0.5rem 0 0 0;
    padding: 0;
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    max-height: 200px;
    overflow-y: auto;
    background: white;
  }
  .history-dropdown li {
    padding: 0.5rem 1rem;
    cursor: pointer;
    border-bottom: 1px solid #f0f0f0;
    font-size: 0.95rem;
  }
  .history-dropdown li:hover, .history-dropdown li.active {
    background: #eef2ff;
  }
</style>