<script setup lang="ts">
/**
 * 输入框 — 双向绑定 v-model
 */
const model = defineModel<string>({ default: '' })

withDefaults(defineProps<{
  placeholder?: string
  clearable?: boolean
  maxlength?: number
}>(), {
  placeholder: '请输入',
  clearable: false,
})

defineEmits<{
  focus: []
  blur: []
}>()
</script>

<template>
  <div class="cmp-input">
    <input
      v-model="model"
      class="cmp-input__inner"
      :placeholder="placeholder"
      :maxlength="maxlength"
      @focus="$emit('focus')"
      @blur="$emit('blur')"
    />
    <button v-if="clearable && model" class="cmp-input__clear" type="button" @click="model = ''">×</button>
  </div>
</template>
