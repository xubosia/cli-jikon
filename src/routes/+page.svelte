<script>
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';// 引入svelte模块,生命变量，在挂载时执行。也就是实现初始化。
  import InputBox from '$lib/input_box/input.svelte';//$lib是svelte的默认路径，表示src/lib目录。
  import myIcon from '$lib/public/x.png';


  // ---------- 状态 ----------
  let files = [];
  let folders = [];
  let apps = [];
  let showFilesView = true;  // 视图控制：当前显示文件还是文件夹
  let navStack = [];//导航栈
  let showAppPicker = false;// 弹窗控制：控制应用选择器的显示
  let pendingFolderPath = '';// 临时变量：存储待打开的文件夹路径
  let appViewActive = false;// 视图模式：是否处于应用视图
  let inputMode = false; // 输入模式：是否正在输入（Tab 切换进入）
  // 历史记录：浏览器和命令输入的历史
  let browserInputHistory = [];
  let commandInputHistory = [];
  let alwaysOnTop = true;// 窗口置顶：控制窗口是否始终在最前面
  let searchMode = false;          // 是否处于搜索结果展示
  let searchResults = [];          // 搜索结果数组
  let searchQuery = '';            // 搜索关键词
  let searchPage = 0;              // 搜索结果页码
  $: searchTotalPages = Math.ceil(searchResults.length / PAGE_SIZE) || 1;

  const PAGE_SIZE = 10; // 分页设置：每页显示 10 个条目
  let rootPage = 0;
  

$: currentLayer = navStack.length > 0
  ? navStack[navStack.length - 1]
  : { path: null, items: appViewActive ? apps : (showFilesView ? files : folders), page: rootPage };//$:响应式变量，当currentLayer改变时，totalPages也会改变。？()：()条件运算符：if-else

  $: totalPages = Math.ceil(currentLayer.items.length / PAGE_SIZE) || 1;
  $: pageItems = currentLayer.items.slice(
    currentLayer.page * PAGE_SIZE,
    (currentLayer.page + 1) * PAGE_SIZE
  );//slice() 方法返回一个从开始到结束（但不包括结束）的数组的浅拷贝。

  $: currentPath = currentLayer.path || (appViewActive ? '应用' : (showFilesView ? '文件' : '文件夹'));

  onMount(async () => {
    try {
      const data = await invoke('load_data');
      files = data.files.map(f => ({ ...f, isDir: f.is_dir })) || [];
      folders = data.folders.map(f => ({ ...f, isDir: f.is_dir })) || [];
      apps = data.apps.map(a => ({ ...a, isDir: a.is_dir })) || [];
      browserInputHistory = data.input_history || [];
      commandInputHistory = data.command_history || [];
    } catch (e) {
      console.error('加载数据失败', e);
    }
    try {
      alwaysOnTop = await invoke('get_always_on_top');
    } catch {}
  });
// 保存数据的函数，为什么是这样的形式呢？因为后端的rust需要的是结构体的形式。map:遍历 → 处理每一项 → 返回新数组
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

  async function addFile() {
    const selected = await open({
      multiple: true,
      filters: [{ name: '所有文件', extensions: ['*'] }]
    });
    if (selected) {
      const newFiles = selected.map(path => ({
        name: path.split(/[\\/]/).pop(),
        path,
        isDir: false
      }));
      files = [...files, ...newFiles];
      await persist();
    }
  }

  async function addFolder() {
    const selected = await open({
      directory: true,
      multiple: false
    });
    if (selected) {
      const name = selected.split(/[\\/]/).pop();
      folders = [...folders, { name, path: selected, isDir: true }];
      await persist();
    }
  }

  async function addApp() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: '可执行文件/快捷方式', extensions: ['exe', 'lnk', 'app', 'desktop', '*'] },
        { name: '所有文件', extensions: ['*'] }
      ]
    });
    if (selected) {
      const path = selected;
      const name = path.split(/[\\/]/).pop();
      apps = [...apps, { name, path, isDir: false }];
      await persist();
    }
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
//这个goback跟返回完全没关系，只是一个重现列表的工具
  function goBack() {
    if (navStack.length > 0) {
      navStack = navStack.slice(0, -1);
    }
  }

async function openItem(path) {
    await invoke('open_path', { path });
    // 如果窗口置顶，在200ms后尝试重新聚焦主窗口（防止被打开的程序挡住）
    if (alwaysOnTop) {
        setTimeout(() => tryFocusWindow(), 200);
    }
}

  async function activateItem(index) {
    const item = pageItems[index];
    if (!item) return;
    if (item.isDir) {
      enterFolder(item.path);// 是目录则进入
    } else {
      await openItem(item.path);
    }
  }

// ---------- 删除（根视图通用） ----------
function deleteItem() {
  // 根视图检查（进入子文件夹时不可删除）
  if (navStack.length > 0) {
    alert('请先返回根视图再删除。');
    return;
  }

  // 获取当前视图对应的列表和数据源名称
  let targetList;
  if (appViewActive) {
    targetList = apps;
  } else {
    targetList = showFilesView ? files : folders;
  }

  // 弹出输入框并解析编号
  const input = prompt('输入要删除的编号（1～9、0）', '');
  if (input === null) return;//input === null 是 严格相等判断 ，作用是： 精准检查变量 input 的值是否为 null（空值），且类型必须完全一致

  const num = parseInt(input, 10);
  if (isNaN(num) || num < 0 || num > 9) {
    alert('请输入有效数字（0～9）');
    return;
  }

  // 编号转索引
  const index = num === 0 ? 9 : num - 1;
  if (index >= pageItems.length || index < 0) {
    alert('该槽位为空。');
    return;
  }

  // 找到要删除的项并从源数组中过滤
  const itemToRemove = pageItems[index];
  if (appViewActive) {
    apps = apps.filter(a => a.path !== itemToRemove.path);
  } else {
    if (showFilesView) {
      files = files.filter(f => f.path !== itemToRemove.path);
    } else {
      folders = folders.filter(f => f.path !== itemToRemove.path);
    }
  }

  persist();
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
// switch 和 default 是 JavaScript 中多路分支的控制结构。

// switch：根据一个表达式的值，执行与之匹配的 case 代码块。

// default：当没有任何 case 的值与表达式匹配时，执行 default 分支（类似 else）。
// break 用于跳出 switch，防止“穿透”到下一个分支。
// default 可以放在任意位置，通常放最后。

async function handleKey(e) {
    // ---------- 搜索模式 ----------
    if (searchMode) {
        if (e.key === 'Escape' || e.key === '/') {
            searchMode = false;
            e.preventDefault();
            return;
        }
        // 按 Enter 打开当前页第一个结果
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
        // 数字键 1~0 选择对应位置的项
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
        // 左右翻页
        if (e.key === 'ArrowLeft' && searchPage > 0) {
            searchPage--;
            e.preventDefault();
        }
        if (e.key === 'ArrowRight' && searchPage < searchTotalPages - 1) {
            searchPage++;
            e.preventDefault();
        }
        return; // 其他键全部忽略
    }

    // 输入框模式下，全局只处理 Tab 切回网格
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

    // 正常模式：按 / 启动搜索
    if (e.key === '/' && !inputMode && !showAppPicker) {
        e.preventDefault();
        startSearch();
        return;
    }

    // 数字快捷键（正常模式）
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
       case 'ArrowLeft':
        if (navStack.length > 0) {
          // 文件夹内部：更新栈顶
          if (currentLayer.page > 0) {
            currentLayer.page--;
            navStack = [...navStack];
            e.preventDefault();
          }
        } else {
          // 根视图：更新 rootPage
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
        case 'Delete':
            deleteItem();
            e.preventDefault();
            break;
        default:
            if (e.key === 'A' && e.shiftKey) {
                showFilesView = !showFilesView;
                navStack = [];
                currentLayer.page = 0;
                lastSelectedIndex = null;
                e.preventDefault();
            } else if (e.key === 'X' && e.shiftKey) {
                appViewActive = !appViewActive;
                navStack = [];
                currentLayer.page = 0;
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
<!-- <svelte:window on:keydown={handleKey} /> -->
 
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
  Shift+A 视图 | Shift+X 应用 | shift+T 置顶 | ←→ 翻页 | Del 删除 |
  a 添加文件 | s 添加文件夹 | d 添加应用 | 
  <br>
  Enter 用应用打开文件夹  Tab 输入框模式 | / 搜索 | 0~9 选择 
  <br>

  后端隐藏功能:在src-tauri\src\hotkey.rs里面

</span>

<div style="text-align: center;  color: var(--text-bright);          /* 纯白，最高亮 */
    font-weight: 600;                   /* 加粗 */
    text-shadow: 0 0 6px var(--shadow-glow);
    background: rgba(0,0,0,0.25);       /* 稍深背景衬托文字 */
    backdrop-filter: blur(4px);font-weight: bold;">

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
    <!-- 原有网格 -->
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
</main>

<!-- <style>
.search-hint {
    color: #ffd966;
    font-size: 0.85rem;
    text-align: center;
    margin-bottom: 0.5rem;
    background: rgba(0,0,0,0.4);
    padding: 0.3rem 0.8rem;
    border-radius: 1rem;
    backdrop-filter: blur(4px);
}
  .icon-img {
      width: 4rem;   /* 调整为你想要的宽度 */
      height: 4rem;  /* 调整为你想要的高度 */
      object-fit: contain;
      filter: drop-shadow(0 2px 3px rgba(0,0,0,0.3));
  }
  /* ---------- 全局重置 ---------- */
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: transparent;
    font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  }

  /* ---------- 主卡片容器 ---------- */
  main {
    width: min(90vw, 800px);
    padding: 2rem;
    background: rgba(255, 255, 255, 0.18);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border-radius: 2rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.5rem;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
    border: 1px solid rgba(255, 255, 255, 0.3);
    margin: auto;
  }

  /* ---------- 操作按钮行 ---------- */
  .actions {
    font-size: 1rem;               /* 大字号 */
    font-weight: 900;  
    display: flex;
    gap: 0.75rem;
    justify-content: center;
    flex-wrap: wrap;
  }

  button {
    background: rgba(74, 144, 217, 0.85);
    color: white;
    border: none;
    padding: 0.6rem 1.4rem;
    border-radius: 2rem;
    font-size: 0.95rem;
    font-weight: 500;
    cursor: pointer;
    backdrop-filter: blur(4px);
    transition: all 0.2s ease;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    letter-spacing: 0.3px;
  }

  button:hover {
    background: #3670b3;
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }

  button:active {
    transform: translateY(0);
  }

  /* ---------- 提示文字行 ---------- */
  .hint-line {
    color: #eee;
    font-size: 0.8rem;
    text-align: center;
    background: rgba(0, 0, 0, 0.25);
    padding: 0.4rem 1.2rem;
    border-radius: 20px;
    backdrop-filter: blur(4px);
    margin-top: -0.5rem;
    white-space: nowrap;
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
    font-weight: 600;
    background: rgba(255, 255, 255, 0.25);
    padding: 0.3rem 0.8rem;
    border-radius: 2rem;
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 0.9rem;
    color: #dbee2c;
  }

  .back-btn {
    background: rgba(255, 255, 255, 0.2);
    color: white;
    border: none;
    padding: 0.3rem 0.8rem;
    border-radius: 2rem;
    cursor: pointer;
    font-size: 0.85rem;
    transition: 0.2s;
  }

  .back-btn:hover {
    background: rgba(255, 255, 255, 0.35);
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
    background: rgba(255, 255, 255, 0.25);
    backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 1rem;
    padding: 0.6rem 0.3rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.3rem;
    cursor: pointer;
    transition: all 0.15s ease;
    overflow: hidden;
  }

  .cell:hover:not(.empty) {
    background: rgba(74, 144, 217, 0.25);
    transform: scale(1.03);
    border-color: rgba(255, 255, 255, 0.6);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.1);
  }

  .cell.empty {
    opacity: 0.35;
    cursor: default;
    background: rgba(255, 255, 255, 0.08);
  }

  .cell.highlighted {
    border-color: #fbbf24;
    box-shadow: 0 0 0 2px rgba(251, 191, 36, 0.5);
  }

  /* ---------- 网格内元素 ---------- */
.key-hint {
  font-weight: 900;
  color: #f2ffb3;
  background: rgba(0, 0, 0, 0.6);
  width: 1.5rem;
  height: 1.5rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 0.25rem;
  font-size: 0.75rem;
  font-family: 'Cascadia Code', 'Consolas', 'Menlo', 'Courier New', monospace;
  text-shadow: 0 0 4px rgba(0, 255, 0, 0.6);
  box-shadow: 0 0 4px rgba(0, 255, 0, 0.2);
  backdrop-filter: blur(4px);
  flex-shrink: 0;
}

  .icon {
    font-size: 1.6rem;
    filter: drop-shadow(0 2px 3px rgba(0,0,0,0.3));
  }

.name {
  font-size: 0.85rem;
  font-weight: 600;
  font-family: 'Cascadia Code', 'Consolas', 'Menlo', 'Courier New', monospace;
  letter-spacing: 0.02em;
  color: #e8f093;
  text-shadow: 0 0 6px rgba(0, 255, 0, 0.5);
  background: rgba(0, 0, 0, 0.5);
  padding: 0.1rem 0.5rem;
  border-radius: 0.2rem;
}

  .empty-text {
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.8rem;
  }

  /* ---------- 翻页 ---------- */
  .pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    font-size: 0.9rem;
    color: #fff;
  }

  .pagination button {
    padding: 0.4rem 1rem;
    font-size: 0.85rem;
  }

  .pagination span {
    background: rgba(0,0,0,0.2);
    padding: 0.3rem 0.8rem;
    border-radius: 1rem;
    backdrop-filter: blur(4px);
  }

  /* ---------- 应用选择器（弹出层） ---------- */
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
    backdrop-filter: blur(6px);
  }

  .picker {
    background: rgba(255, 255, 255, 0.9);
    backdrop-filter: blur(15px);
    padding: 1.5rem;
    border-radius: 1.5rem;
    min-width: 300px;
    max-width: 90%;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.5);
  }

  .picker h3 {
    margin: 0 0 1rem 0;
    font-size: 1.1rem;
    color: #111;
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
    padding: 0.5rem 0.2rem;
    cursor: pointer;
    border-bottom: 1px solid rgba(0,0,0,0.1);
    transition: 0.1s;
    color: #222;
  }

  .picker li:hover {
    background: rgba(74, 144, 217, 0.15);
    border-radius: 0.5rem;
  }



  .esc-hint {
    text-align: right;
    color: #666;
    font-size: 0.8rem;
    margin: 0.5rem 0 0 0;
  }

  /* ---------- 输入框模式（保留基本样式，具体在组件内） ---------- */
</style> -->



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

    --bg-card: hsla(var(--hue), var(--sat), 85%, 0.1);
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
    width: min(90vw, 800px);
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