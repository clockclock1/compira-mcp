<script setup lang="ts">
/**
 * 标签 Tag
 */
withDefaults(defineProps<{
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info'
  closable?: boolean
}>(), {
  type: 'info',
  closable: false,
})

defineEmits<{
  close: []
}>()
</script>

<template>
  <span class="cmp-tag" :class="`cmp-tag--${type}`">
    <slot />
    <button v-if="closable" type="button" class="cmp-tag__close" @click="$emit('close')">×</button>
  </span>
</template>
