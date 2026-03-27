<template>
  <div class="app-container">
    <router-view />
  </div>
</template>

<script setup lang="ts">
// App.vue 现在只作为路由容器，不包含具体业务逻辑
</script>

<style scoped>
.app-container {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
</style>
