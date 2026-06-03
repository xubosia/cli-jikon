<script>
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import InputBox from '$lib/input_box/input.svelte';
  import { listen } from '@tauri-apps/api/event';
  import { openUrl } from '@tauri-apps/plugin-opener';

  // ---------- 动态图标库（从 src/lib/public 读取所有图片并按文件名排序）----------
 const imageModules = import.meta.glob('$lib/public/*.{png,jpg,webp,gif,jpeg}', {
  eager: true,
  query: 'url'
});
const rawIconList = Object.entries(imageModules)
  .map(([key, mod]) => {
    const name = key.split('/').pop();      // 文件名
    const url = mod.default || mod;         // 取出 URL（mod.default 是真实 URL）
    return { name, url };
  })
  .sort((a, b) => a.name.localeCompare(b.name));
const iconList = rawIconList.map(e => e.url);   // 只保留 URL 数组
let nextIconIndex = 0;
  // ---------- 状态 ----------
  let files = [];
  let folders = [];
  let apps = [];
  let showFilesView = true;
  let navStack = [];
  let showAppPicker = false;
  let pendingFolderPath = '';
  let appViewActive = false;
  let inputMode = false;
  let browserInputHistory = [];
  let commandInputHistory = [];
  let alwaysOnTop = true;
  let searchMode = false;
  let searchResults = [];
  let searchQuery = '';
  let searchPage = 0;
  $: searchTotalPages = Math.ceil(searchResults.length / PAGE_SIZE) || 1;

  const PAGE_SIZE = 10;
  let rootPage = 0;

  // 右键菜单状态
  let contextMenu = { show: false, x: 0, y: 0, item: null };

  let currentEngineIdx = 0;

  // 辅助函数：根据当前搜索引擎打开或搜索
  async function openUrlOrSearch(raw) {
    let url;
    if (raw.startsWith('http://') || raw.startsWith('https://')) {
      url = raw;
    } else if (raw.startsWith('www.')) {
      url = 'http://' + raw;
    } else if (raw.includes('.') && !raw.includes(' ') && !raw.includes('\t')) {
      url = 'http://' + raw;
    } else {
      const engines = await invoke('get_search_engines');
      const engine = engines[currentEngineIdx] || engines[0];
      url = engine.url_template.replace('{q}', encodeURIComponent(raw));
    }
    await openUrl(url);
  }

  $: currentLayer = navStack.length > 0
    ? navStack[navStack.length - 1]
    : { path: null, items: appViewActive ? apps : (showFilesView ? files : folders), page: rootPage };

  $: totalPages = Math.ceil(currentLayer.items.length / PAGE_SIZE) || 1;
  $: pageItems = currentLayer.items.slice(
    currentLayer.page * PAGE_SIZE,
    (currentLayer.page + 1) * PAGE_SIZE
  );

  $: currentPath = currentLayer.path || (appViewActive ? '应用' : (showFilesView ? '文件' : '文件夹'));

onMount(async () => {
    try {
      const data = await invoke('load_data');
      files = data.files.map(f => ({ ...f, isDir: f.is_dir, icon: f.icon || null })) || [];
      folders = data.folders.map(f => ({ ...f, isDir: f.is_dir })) || [];
      apps = data.apps.map(a => ({ ...a, isDir: a.is_dir, icon: a.icon || null })) || [];
      browserInputHistory = data.input_history || [];
      commandInputHistory = data.command_history || [];

      // 修复历史数据：给没有图标的文件/应用补图标
      ensureAllIcons();
    } catch (e) {
      console.error('加载数据失败', e);
    }
    try {
      alwaysOnTop = await invoke('get_always_on_top');
    } catch {}
    
  let pendingClipboardAction = null;
  listen('clipboard-action', async (event) => {
    pendingClipboardAction = event;
    await new Promise(resolve => setTimeout(resolve, 300));   // 等待 300ms
    if (pendingClipboardAction !== event) return;             // 不是最新事件，丢弃
    const { text, mode } = event.payload;
    await invoke('show_main_window').catch(() => {});
    if (mode === 'browser') {
        await openUrlOrSearch(text);
        browserInputHistory = [text, ...browserInputHistory.filter(h => h !== text)];
    } else {
        await invoke('execute_command', { cmd: text });
        commandInputHistory = [text, ...commandInputHistory.filter(h => h !== text)];
    }
    persist();
    pendingClipboardAction = null;
});
  });
  async function persist() {
    try {
      await invoke('save_data', {
        data: {
          files: files.map(({ isDir, ...rest }) => ({ ...rest, is_dir: isDir })),
          folders: folders.map(({ isDir, ...rest }) => ({ ...rest, is_dir: isDir })),
          apps: apps.map(({ isDir, ...rest }) => ({ ...rest, is_dir: isDir })),
          input_history: browserInputHistory,
          command_history: commandInputHistory,
        }
      });
    } catch (e) {
      console.error('保存数据失败', e);
    }
  }

  // 通用添加（自动分配图标）
  async function addEntries(type, isDir, openOptions) {
    const selected = await open(openOptions);
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    const newItems = paths.map(path => {
      const raw = path.split(/[\\/]/).pop();
      const dot = raw.lastIndexOf('.');
      const name = dot > 0 ? raw.substring(0, dot) : raw;   // 去后缀
      // 为文件/应用按顺序分配图标，文件夹不分配
      const icon = (!isDir && iconList.length > 0) ? iconList[nextIconIndex % iconList.length] : null;
      if (!isDir && iconList.length > 0) nextIconIndex = (nextIconIndex + 1) % iconList.length;
      return { name, path, isDir, icon };
    });

    if (type === 'files') {
      files = [...files, ...newItems];
    } else if (type === 'folders') {
      folders = [...folders, ...newItems];
    } else if (type === 'apps') {
      apps = [...apps, ...newItems];
    }
    await persist();
  }

  function ensureAllIcons() {
    let changed = false;
    const allLists = [files, folders, apps];

    for (const list of allLists) {
        for (const item of list) {
            // 只处理非文件夹、当前没有图标的条目
            if (!item.isDir && !item.icon && iconList.length > 0) {
                item.icon = iconList[nextIconIndex % iconList.length];
                nextIconIndex = (nextIconIndex + 1) % iconList.length;
                changed = true;
            }
        }
    }

    if (changed) {
        // 立即保存，下次启动就不需要再补了
        persist();
        // 触发 Svelte 响应式更新（直接修改属性不会触发重渲染）
        files = files;
        folders = folders;
        apps = apps;
    }
}

  async function addFile() {
    await addEntries('files', false, {
      multiple: true,
      filters: [{ name: '所有文件', extensions: ['*'] }]
    });
  }

  async function addFolder() {
    await addEntries('folders', true, {
      directory: true,
      multiple: false
    });
  }

  async function addApp() {
    await addEntries('apps', false, {
      multiple: false,
      filters: [
        { name: '可执行文件/快捷方式', extensions: ['exe', 'lnk', 'app', 'desktop', '*'] },
        { name: '所有文件', extensions: ['*'] }
      ]
    });
  }

  async function enterFolder(folderPath) {
    const entries = await invoke('read_dir', { path: folderPath });
    const items = entries.map(e => ({
      name: e.name,
      path: e.path,
      isDir: e.is_dir
    }));
    navStack = [...navStack, { path: folderPath, items, page: 0 }];
  }

  function goBack() {
    if (navStack.length > 0) {
      navStack = navStack.slice(0, -1);
    }
  }

  async function openItem(path) {
    await invoke('open_path', { path });
    if (alwaysOnTop) {
      setTimeout(() => tryFocusWindow(), 200);
    }
  }

  async function activateItem(index) {
    const item = pageItems[index];
    if (!item) return;
    if (item.isDir) {
      enterFolder(item.path);
    } else {
      await openItem(item.path);
    }
  }

  function showPickerForFolder(folderPath) {
    if (apps.length === 0) {
      alert('请先添加应用（通过“＋ 添加应用”按钮）。');
      return;
    }
    pendingFolderPath = folderPath;
    showAppPicker = true;
  }

  async function selectApp(index) {
    const app = apps[index];
    if (!app) return;
    await invoke('open_with', { folderPath: pendingFolderPath, appPath: app.path });
    showAppPicker = false;
    if (alwaysOnTop) {
      setTimeout(() => tryFocusWindow(), 200);
    }
  }

  function closePicker() {
    showAppPicker = false;
  }

  function tryFocusWindow(attempts = 3, interval = 300) {
    if (attempts <= 0) return;
    invoke('focus_main_window').catch(() => {});
    setTimeout(() => tryFocusWindow(attempts - 1, interval), interval);
  }

  function startSearch() {
    const query = prompt('在当前目录下搜索：', '')?.trim().toLowerCase();
    if (!query) return;
    const results = currentLayer.items.filter(item => item.name.toLowerCase().includes(query));
    if (results.length === 0) {
      alert('没有找到匹配项');
      return;
    }
    searchResults = results;
    searchQuery = query;
    searchPage = 0;
    searchMode = true;
  }

  // ---------- 右键菜单逻辑 ----------
  function onContextMenu(e, item) {
    e.preventDefault();
    if (navStack.length > 0 || !item) return;
    contextMenu = { show: true, x: e.clientX, y: e.clientY, item };
  }

  function closeContextMenu() {
    contextMenu.show = false;
  }

  function getContextMenuItems(item) {
    if (!item) return [];
    return [
      {
        label: '✏️ 重命名',
        action: () => {
          const oldName = item.name;
          const newName = prompt('重命名（仅修改显示名称）:', oldName);
          if (newName === null || newName.trim() === '' || newName.trim() === oldName) return;
          item.name = newName.trim();
          if (appViewActive) apps = apps;
          else if (showFilesView) files = files;
          else folders = folders;
          persist();
        }
      },
      {
        label: '🗑️ 删除',
        action: () => {
          if (appViewActive) {
            apps = apps.filter(a => a.path !== item.path);
          } else {
            if (showFilesView) {
              files = files.filter(f => f.path !== item.path);
            } else {
              folders = folders.filter(f => f.path !== item.path);
            }
          }
          persist();
        }
      }
    ];
  }

  async function handleKey(e) {
    // ---------- 右键菜单模式优先 ----------
    if (contextMenu.show) {
      if (e.key === 'Escape') {
        closeContextMenu();
        e.preventDefault();
        return;
      }
      const menuItems = getContextMenuItems(contextMenu.item);
      if (/^[0-9]$/.test(e.key)) {
        const idx = e.key === '0' ? 9 : Number(e.key) - 1;
        if (idx < menuItems.length) {
          menuItems[idx].action();
          closeContextMenu();
          e.preventDefault();
        }
        return;
      }
      e.preventDefault();
      return;
    }

    // ---------- 搜索模式 ----------
    if (searchMode) {
      if (e.key === 'Escape' || e.key === '/') {
        searchMode = false;
        e.preventDefault();
        return;
      }
      if (e.key === 'Enter') {
        const first = searchResults[searchPage * PAGE_SIZE];
        if (first) {
          if (first.isDir) await enterFolder(first.path);
          else await openItem(first.path);
          searchMode = false;
        }
        e.preventDefault();
        return;
      }
      if (/^[0-9]$/.test(e.key)) {
        const idx = e.key === '0' ? 9 : Number(e.key) - 1;
        const globalIdx = searchPage * PAGE_SIZE + idx;
        if (globalIdx < searchResults.length) {
          const item = searchResults[globalIdx];
          if (item.isDir) await enterFolder(item.path);
          else await openItem(item.path);
          searchMode = false;
        }
        e.preventDefault();
        return;
      }
      if (e.key === 'ArrowLeft' && searchPage > 0) {
        searchPage--;
        e.preventDefault();
      }
      if (e.key === 'ArrowRight' && searchPage < searchTotalPages - 1) {
        searchPage++;
        e.preventDefault();
      }
      return;
    }

    // 输入框模式
    if (inputMode) {
      if (e.key === 'Tab') {
        inputMode = false;
        e.preventDefault();
      }
      return;
    }

    // 应用选择器模式
    if (showAppPicker) {
      if (e.key === 'Escape') {
        closePicker();
        e.preventDefault();
        return;
      }
      if (/^[0-9]$/.test(e.key)) {
        const realIdx = e.key === '0' ? 9 : Number(e.key) - 1;
        if (realIdx < apps.length) await selectApp(realIdx);
        e.preventDefault();
      }
      return;
    }

    // 正常模式：启动搜索
    if (e.key === '/' && !inputMode && !showAppPicker) {
      e.preventDefault();
      startSearch();
      return;
    }

    // 数字快捷键
    if (/^[0-9]$/.test(e.key)) {
      const idx = e.key === '0' ? 9 : Number(e.key) - 1;
      lastSelectedIndex = idx;
      await activateItem(idx);
      e.preventDefault();
      return;
    }

    switch (e.key) {
      case 'Tab':
        inputMode = true;
        e.preventDefault();
        break;
      case 'Enter': {
        if (appViewActive) return;
        e.preventDefault();
        showPickerForFolder(currentPath);
        break;
      }
       case 'f':
          // 仅在文件夹视图下，且有真实文件路径时才有效
          if (!appViewActive && currentLayer.path) {
            await invoke('open_path', { path: currentLayer.path });
          }
          e.preventDefault();
          break;
      case 'ArrowLeft':
        if (navStack.length > 0) {
          if (currentLayer.page > 0) {
            currentLayer.page--;
            navStack = [...navStack];
            e.preventDefault();
          }
        } else {
          if (rootPage > 0) {
            rootPage--;
            e.preventDefault();
          }
        }
        break;
      case 'ArrowRight':
        if (navStack.length > 0) {
          if (currentLayer.page < totalPages - 1) {
            currentLayer.page++;
            navStack = [...navStack];
            e.preventDefault();
          }
        } else {
          if (rootPage < totalPages - 1) {
            rootPage++;
            e.preventDefault();
          }
        }
        break;
      case 'Backspace':
        goBack();
        e.preventDefault();
        break;
      default:
        if (e.key === 'A' && e.shiftKey) {
          showFilesView = !showFilesView;
          navStack = [];
          rootPage = 0;
          lastSelectedIndex = null;
          e.preventDefault();
        } else if (e.key === 'X' && e.shiftKey) {
          appViewActive = !appViewActive;
          navStack = [];
          rootPage = 0;
          lastSelectedIndex = null;
          e.preventDefault();
        } else if (e.key === 'T' && e.shiftKey) {
          alwaysOnTop = await invoke('toggle_always_on_top');
          e.preventDefault();
        } else if (e.key === 'a') {
          await addFile();
          e.preventDefault();
        } else if (e.key === 's') {
          await addFolder();
          e.preventDefault();
        } else if (e.key === 'd') {
          await addApp();
          e.preventDefault();
        }
        break;
    }
  }

  let lastSelectedIndex = null;
</script>

    <svelte:window on:keydown|capture={handleKey} />

<main>
  {#if inputMode}
    <InputBox
      browserHistory={browserInputHistory}
      commandHistory={commandInputHistory}
      alwaysOnTop={alwaysOnTop}
      onExternalOpen={() => {
        if (alwaysOnTop) {
            setTimeout(() => tryFocusWindow(), 200);
        }
      }}
      onHistoryUpdate={(mode, newHistory) => {
        if (mode === 'browser') {
          browserInputHistory = newHistory;
        } else {
          commandInputHistory = newHistory;
        }
        persist();
      }}
      onSwitchMode={() => (inputMode = false)}
    />
  {:else}
    <div class="actions">
        <button on:click={addFile}>📄 添加文件</button>
        <button on:click={addFolder}>📁 添加文件夹</button>
        <button on:click={addApp}>⚙️ 添加应用</button>
        <span class="hint">
          {#if apps.length === 0}
            ⚠️ 未添加应用，Enter 无效 |
          {/if}
          Shift+A 视图 | Shift+X 应用 | shift+T 置顶 | ←→ 翻页 |
          a 添加文件 | s 添加文件夹 | d 添加应用 | 
          <br>
          Enter 用应用打开文件夹  Tab 输入框模式 | / 搜索 | 0~9 选择 | 右键菜单
        </span>
        <div style="text-align: center; color: var(--text-bright); font-weight: 600; text-shadow: 0 0 6px var(--shadow-glow); background: rgba(0,0,0,0.25); backdrop-filter: blur(4px);">
          {alwaysOnTop ? '(置顶模式)' : ''}
        </div>
    </div>

    <div class="breadcrumb">
      <span class="path">{currentPath}</span>
      {#if navStack.length > 0}
        <button class="back-btn" on:click={goBack}>⬅ 返回上级</button>
      {/if}
    </div>

    <div class="grid">
      {#if searchMode}
        <div class="search-hint">
          搜索 “{searchQuery}” → {searchResults.length} 个结果 &nbsp;|&nbsp;
          第 {searchPage + 1}/{searchTotalPages} 页 &nbsp;|&nbsp;
          Esc 退出 | Enter 打开第一个 | 1~0 选择
        </div>
        {#each [0, 1] as row}
          <div class="row">
            {#each [0, 1, 2, 3, 4] as col}
              {@const index = row * 5 + col}
              {@const item = searchResults[searchPage * PAGE_SIZE + index]}
              <div
                class="cell"
                class:empty={!item}
                on:click={() => {
                  if (!item) return;
                  if (item.isDir) enterFolder(item.path);
                  else openItem(item.path);
                  searchMode = false;
                }}
                on:contextmenu={(e) => onContextMenu(e, item)}
                role="button"
                tabindex="-1"
              >
                {#if item}
                  <span class="key-hint">{index + 1 === 10 ? 0 : index + 1}</span>
                  {#if item.isDir}
                    <span class="icon">📁</span>
                  {:else}
                    <img class="icon-img" src={myIcon} alt="" />
                  {/if}
                  <span class="name">{item.name}</span>
                {:else}
                  <span class="key-hint">{index + 1 === 10 ? 0 : index + 1}</span>
                  <span class="empty-text">空</span>
                {/if}
              </div>
            {/each}
          </div>
        {/each}
      {:else}
        {#each [0, 1] as row}
          <div class="row">
            {#each [0, 1, 2, 3, 4] as col}
              {@const index = row * 5 + col}
              {@const item = pageItems[index]}
              <div
                class="cell"
                class:empty={!item}
                class:highlighted={lastSelectedIndex === index}
                on:click={() => { lastSelectedIndex = index; activateItem(index); }}
                on:contextmenu={(e) => onContextMenu(e, item)}
                role="button"
                tabindex="-1"
              >
                {#if item}
                  <span class="key-hint">{index + 1 === 10 ? 0 : index + 1}</span>
             {#if item.isDir}
                <span class="icon">📁</span>
              {:else if item.icon}
                <img class="icon-img" src={item.icon} alt="" />
              {:else}
                <span class="icon">📄</span>
              {/if}
                  <span class="name">{item.name}</span>
                {:else}
                  <span class="key-hint">{index + 1 === 10 ? 0 : index + 1}</span>
                  <span class="empty-text">空</span>
                {/if}
              </div>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

    <div class="pagination">
      <button
        on:click={() => { currentLayer.page--; navStack = navStack; }}
        disabled={currentLayer.page <= 0}
      >← 上一页</button>
      <span>第 {currentLayer.page + 1}/{totalPages} 页</span>
      <button
        on:click={() => { currentLayer.page++; navStack = navStack; }}
        disabled={currentLayer.page >= totalPages - 1}
      >下一页 →</button>
    </div>

    {#if showAppPicker}
      <div class="overlay" on:click={closePicker}>
        <div class="picker" on:click|stopPropagation>
          <h3>选择应用打开文件夹</h3>
          <ul>
            {#each apps as app, i}
              <li on:click={() => selectApp(i)} class:active={i === apps.length - 1}>
                <span class="key-hint">{i + 1 === 10 ? 0 : i + 1}</span>
                <span>{app.name}</span>
              </li>
            {/each}
          </ul>
          <p class="esc-hint">按 Esc 关闭</p>
        </div>
      </div>
    {/if}
  {/if}

  <!-- 右键菜单浮层 -->
  {#if contextMenu.show}
    <div class="context-overlay" on:click={closeContextMenu} on:contextmenu={closeContextMenu}>
      <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
        {#each getContextMenuItems(contextMenu.item) as menuItem, i}
          <button
            class="context-item"
            on:click={() => { menuItem.action(); closeContextMenu(); }}
          >
            <span class="key-hint">{i + 1 === 10 ? 0 : i + 1}</span>
            {menuItem.label}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</main>

<style>
  /* ============================================
     仙气主题 · 变量驱动 · 一键换色
     修改 --hue 的值即可：
       260 = 紫色   220 = 蓝    180 = 青
       45  = 金色     0 = 红    120 = 绿
  ============================================ */
  :root {
    --hue: 210;
    --sat: 55%;

    --bg-card: hsla(var(--hue), var(--sat), 85%, 0.2);
    --bg-cell: hsla(var(--hue), var(--sat), 88%, 0.06);
    --bg-overlay: rgba(20, 10, 30, 0.4);

    --text-main: hsl(var(--hue), 45%, 95%);   /* 更亮 + 更高饱和度 */
    --text-soft: hsl(var(--hue), 40%, 90%);
    --text-bright: #ffffff;                   /* 纯白保持 */

    /* 可选：让阴影更深，增加立体感 */
    --shadow-glow: hsla(var(--hue), 70%, 60%, 0.35);

    --border-light: hsla(var(--hue), 50%, 90%, 0.2);
    --border-strong: hsla(var(--hue), 60%, 95%, 0.5);

    --shadow-glow: hsla(var(--hue), 70%, 75%, 0.25);
    --shadow-card: 0 20px 50px rgba(0,0,0,0.15), 0 0 30px hsla(var(--hue), 60%, 70%, 0.2);
    --shadow-cell-hover: 0 10px 25px hsla(var(--hue), 70%, 70%, 0.25), 0 0 20px hsla(var(--hue), 60%, 85%, 0.3);

    --btn-bg: rgba(255,255,255,0.12);
    --btn-hover: rgba(255,255,255,0.22);

    --radius-card: 2.5rem;
    --radius-cell: 1.2rem;
    --radius-btn: 2rem;
  }

  /* ============================================
     通用组件
  ============================================ */
  .search-hint {
    color: var(--text-soft);
    font-size: 0.85rem;
    text-align: center;
    margin-bottom: 0.5rem;
    background: rgba(255,255,255,0.06);
    padding: 0.4rem 1rem;
    border-radius: var(--radius-btn);
    backdrop-filter: blur(12px);
    box-shadow: 0 0 15px var(--shadow-glow);
    border: 1px solid var(--border-light);
  }

  .icon-img {
    width: 4rem;
    height: 4rem;
    object-fit: contain;
    filter: drop-shadow(0 4px 6px hsla(var(--hue), 60%, 70%, 0.3));
    transition: transform 0.2s ease;
  }

  /* ---------- 全局重置 ---------- */
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: transparent;
    font-family: 'Inter', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  }

  /* ---------- 主卡片容器 ---------- */
  main {
    position: relative;
    width: min(90vw, 1000px);
    padding: 2rem;
    background: var(--bg-card);
    backdrop-filter: blur(24px) saturate(120%);
    -webkit-backdrop-filter: blur(24px) saturate(120%);
    border-radius: var(--radius-card);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.5rem;
    box-shadow: var(--shadow-card);
    border: 1px solid var(--border-light);
    margin: auto;
    transition: all 0.3s ease;
  }

  main::before {
  content: "";
  position: absolute;
  inset: 0; /* 覆盖整个main容器 */
  background: url("D:\change-world\programme\rust\tauri-app\src\lib\public\7914c94280454b41afbc8c82c180c688.png") center/cover no-repeat;
  opacity: 0.05; /* 这里控制图片透明度，0-1之间，数值越小越透明 */
  border-radius: inherit; /* 继承main的圆角 */
  z-index: -1; /* 放在内容下面 */
}

  /* ---------- 操作按钮行 ---------- */
  .actions {
    font-size: 1rem;
    font-weight: 600;
    display: flex;
    gap: 0.75rem;
    justify-content: center;
    flex-wrap: wrap;
  }

  button {
    background: var(--btn-bg);
    color: var(--text-main);
    border: 1px solid var(--border-light);
    padding: 0.6rem 1.4rem;
    border-radius: var(--radius-btn);
    font-size: 0.95rem;
    font-weight: 500;
    cursor: pointer;
    backdrop-filter: blur(10px);
    transition: all 0.25s ease;
    box-shadow: 0 4px 12px rgba(0,0,0,0.1), 0 0 8px var(--shadow-glow);
    letter-spacing: 0.4px;
  }

  button:hover {
    background: var(--btn-hover);
    border-color: var(--border-strong);
    color: var(--text-bright);
    transform: translateY(-2px);
    box-shadow: 0 8px 20px hsla(var(--hue), 60%, 55%, 0.25), 0 0 15px hsla(var(--hue), 60%, 85%, 0.4);
  }

  button:active {
    transform: translateY(0);
    box-shadow: 0 2px 6px rgba(0,0,0,0.15);
  }

  /* ---------- 提示文字行 ---------- */
  .hint {
    display: inline-block;
    max-width: 100%;
    text-align: center;
     /* 原来 */
  /* color: var(--text-soft); */

  /* 改为 */
    color: var(--text-bright);          /* 纯白，最高亮 */
    font-weight: 600;                   /* 加粗 */
    text-shadow: 0 0 6px var(--shadow-glow);
    background: rgba(0,0,0,0.25);       /* 稍深背景衬托文字 */
    backdrop-filter: blur(4px);
  

    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.04em;
    line-height: 1.6;
    background: rgba(255, 255, 255, 0.06);
    backdrop-filter: blur(10px);
    padding: 0.5rem 1.2rem;
    border-radius: 99px;
    border: 1px solid var(--border-light);
    box-shadow: 0 0 15px var(--shadow-glow);
    text-shadow: 0 1px 4px hsla(var(--hue), 60%, 60%, 0.3);
  }

  /* ---------- 面包屑导航 ---------- */
  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    margin-bottom: -0.5rem;
  }

  .path {
    font-weight: 500;
    background: rgba(255,255,255,0.06);
    padding: 0.3rem 0.8rem;
    border-radius: var(--radius-btn);
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 0.9rem;
 /* 原来 */
  /* color: var(--text-soft); */

  /* 改为 */
  color: var(--text-bright);          /* 纯白，最高亮 */
  font-weight: 600;                   /* 加粗 */
  text-shadow: 0 0 6px var(--shadow-glow);
  background: rgba(0,0,0,0.25);       /* 稍深背景衬托文字 */
  backdrop-filter: blur(4px);

    border: 1px solid var(--border-light);
    text-shadow: 0 1px 4px hsla(var(--hue), 60%, 60%, 0.3);
    backdrop-filter: blur(6px);
  }

  .back-btn {
    background: rgba(255,255,255,0.1);
    color: var(--text-main);
    border: 1px solid var(--border-light);
    padding: 0.3rem 0.8rem;
    border-radius: var(--radius-btn);
    cursor: pointer;
    font-size: 0.85rem;
    transition: 0.25s ease;
    backdrop-filter: blur(8px);
    box-shadow: 0 2px 8px rgba(0,0,0,0.1);
  }

  .back-btn:hover {
    background: rgba(255,255,255,0.2);
    border-color: var(--border-strong);
    box-shadow: 0 4px 15px var(--shadow-glow);
  }

  /* ---------- 网格区域 ---------- */
  .grid {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .row {
    display: flex;
    gap: 0.75rem;
    justify-content: center;
  }

  .cell {
    flex: 1 1 0;
    max-width: 130px;
    height: 110px;
    background: var(--bg-cell);
    backdrop-filter: blur(10px);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-cell);
    padding: 0.6rem 0.3rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.3rem;
    cursor: pointer;
    transition: all 0.2s ease;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(0,0,0,0.08);
  }

  .cell:hover:not(.empty) {
    background: rgba(255,255,255,0.12);
    transform: translateY(-3px) scale(1.02);
    border-color: var(--border-strong);
    box-shadow: var(--shadow-cell-hover);
  }

  .cell.empty {
    opacity: 0.2;
    cursor: default;
    background: rgba(255,255,255,0.03);
    border-color: rgba(255,255,255,0.08);
  }

  .cell.highlighted {
    border-color: hsla(var(--hue), 80%, 85%, 0.7);
    box-shadow: 0 0 18px hsla(var(--hue), 80%, 75%, 0.5);
  }

  /* ---------- 网格内元素 ---------- */
  .key-hint {
    font-weight: 700;
    color: var(--text-bright);
    background: rgba(255,255,255,0.15);
    width: 1.6rem;
    height: 1.6rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 0.4rem;
    font-size: 0.75rem;
    font-family: 'Cascadia Code', 'Consolas', monospace;
    text-shadow: 0 1px 4px hsla(var(--hue), 60%, 60%, 0.4);
    backdrop-filter: blur(6px);
    border: 1px solid var(--border-light);
    box-shadow: 0 2px 6px rgba(0,0,0,0.2);
    flex-shrink: 0;
  }

  .icon {
    font-size: 1.8rem;
    filter: drop-shadow(0 4px 6px hsla(var(--hue), 60%, 70%, 0.3));
    transition: transform 0.2s;
  }

  .cell:hover .icon {
    transform: scale(1.05);
  }

  .name {
    font-size: 0.8rem;
    font-weight: 600;
    font-family: 'Cascadia Code', 'Consolas', 'Menlo', monospace;
    letter-spacing: 0.03em;
    color: var(--text-main);
    text-shadow: 0 1px 6px hsla(var(--hue), 60%, 65%, 0.6);
    background: rgba(0,0,0,0.25);
    padding: 0.15rem 0.5rem;
    border-radius: 0.3rem;
    backdrop-filter: blur(4px);
    border: 1px solid rgba(255,255,255,0.1);
  }

  .empty-text {
    color: rgba(255,255,255,0.35);
    font-size: 0.8rem;
    text-shadow: 0 1px 2px rgba(0,0,0,0.3);
  }

  /* ---------- 翻页 ---------- */
  .pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    font-size: 0.9rem;
    color: var(--text-soft);
  }

  .pagination button {
    padding: 0.4rem 1rem;
    font-size: 0.85rem;
    background: rgba(255,255,255,0.1);
    border: 1px solid var(--border-light);
  }

  .pagination span {
    background: rgba(255,255,255,0.06);
    padding: 0.3rem 0.8rem;
    border-radius: var(--radius-btn);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border-light);
    color: var(--text-main);
    text-shadow: 0 1px 4px hsla(var(--hue), 60%, 60%, 0.4);
  }

  /* ---------- 应用选择器（弹出层） ---------- */
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: var(--bg-overlay);
    backdrop-filter: blur(12px);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .picker {
    background: rgba(255,255,255,0.08);
    backdrop-filter: blur(25px) saturate(150%);
    -webkit-backdrop-filter: blur(25px) saturate(150%);
    padding: 2rem;
    border-radius: var(--radius-card);
    min-width: 300px;
    max-width: 90%;
    box-shadow: 0 25px 60px rgba(0,0,0,0.3), 0 0 30px hsla(var(--hue), 70%, 70%, 0.3);
    border: 1px solid var(--border-light);
    color: var(--text-main);
  }

  .picker h3 {
    margin: 0 0 1.5rem 0;
    font-size: 1.2rem;
    color: var(--text-bright);
    text-shadow: 0 2px 8px hsla(var(--hue), 70%, 70%, 0.5);
  }

  .picker ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .picker li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 0.5rem;
    cursor: pointer;
    border-bottom: 1px solid rgba(255,255,255,0.08);
    transition: 0.2s;
    color: var(--text-soft);
    border-radius: 0.8rem;
    margin-bottom: 0.2rem;
  }

  .picker li:hover {
    background: rgba(255,255,255,0.15);
    box-shadow: 0 4px 12px rgba(0,0,0,0.2);
    color: var(--text-bright);
    border-color: transparent;
  }

  .picker li .key-hint {
    background: rgba(255,255,255,0.2);
    color: var(--text-bright);
    font-weight: bold;
    box-shadow: 0 0 8px var(--shadow-glow);
  }

  .esc-hint {
    text-align: right;
    color: rgba(255,255,255,0.5);
    font-size: 0.8rem;
    margin: 0.8rem 0 0 0;
    text-shadow: 0 1px 2px rgba(0,0,0,0.2);
  }
  

  /* ---------- 输入框模式（保留基本样式，具体在组件内） ---------- */
</style>