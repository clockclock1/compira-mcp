<script setup lang="ts">
/**
 * 基础按钮 — 支持类型与尺寸
 */
withDefaults(defineProps<{
  label?: string
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'default'
  size?: 'small' | 'medium' | 'large'
  disabled?: boolean
}>(), {
  label: '按钮',
  type: 'default',
  size: 'medium',
  disabled: false,
})

const emit = defineEmits<{
  click: [event: MouseEvent]
}>()
</script>

<template>
  <button
    class="cmp-btn"
    :class="[`cmp-btn--${type}`, `cmp-btn--${size}`]"
    :disabled="disabled"
    @click="emit('click', $event)"
  >
    <slot>{{ label }}</slot>
  </button>
</template>
