<script setup lang="ts">
/**
 * 开关 Switch
 */
const model = defineModel<boolean>({ default: false })

defineProps<{
  activeText?: string
  inactiveText?: string
  disabled?: boolean
}>()

defineEmits<{
  change: [value: boolean]
}>()
</script>

<template>
  <label class="cmp-switch" :class="{ 'is-checked': model, 'is-disabled': disabled }">
    <input
      type="checkbox"
      :checked="model"
      :disabled="disabled"
      @change="model = ($event.target as HTMLInputElement).checked; $emit('change', model)"
    />
    <span class="cmp-switch__core" />
    <span v-if="activeText || inactiveText" class="cmp-switch__label">
      {{ model ? activeText : inactiveText }}
    </span>
  </label>
</template>
