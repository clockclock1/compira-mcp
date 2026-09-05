<script setup lang="ts">
/**
 * 卡片容器
 */
defineProps<{
  title?: string
  shadow?: 'always' | 'hover' | 'never'
}>()
</script>

<template>
  <div class="cmp-card" :class="[`is-shadow-${shadow || 'always'}`]">
    <header v-if="title || $slots.header" class="cmp-card__header">
      <slot name="header">{{ title }}</slot>
    </header>
    <div class="cmp-card__body">
      <slot />
    </div>
    <footer v-if="$slots.footer" class="cmp-card__footer">
      <slot name="footer" />
    </footer>
  </div>
</template>
