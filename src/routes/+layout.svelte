<script>
  import '../app.css';
</script>

<div class="app-shell">
  <slot />
</div>

<style>
  .app-shell {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;      /* 水平居中 */
    justify-content: center;  /* 垂直居中 */
    background: transparent;
  }
</style>