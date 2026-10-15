<script setup lang="ts">
import { useId } from 'vue'

const { label, error, helper, hideLabel = false } = defineProps<{
  label: string
  error?: string
  helper?: string
  hideLabel?: boolean
}>()

const fieldId = useId()
const messageId = `${fieldId}-message`
</script>

<template>
  <label class="ui-form-field" :class="{ 'has-message': error || helper, 'hide-label': hideLabel }" :for="fieldId">
    <span class="ui-form-field-label" :class="{ 'is-hidden': hideLabel }">{{ label }}</span>
    <slot
      :field-id="fieldId"
      :message-id="messageId"
      :described-by="error || helper ? messageId : undefined"
      :invalid="Boolean(error)"
    />
    <small
      v-if="error || helper"
      :id="messageId"
      :class="{ error: Boolean(error) }"
      :aria-live="error ? 'polite' : undefined"
    >
      {{ error || helper }}
    </small>
  </label>
</template>

<style scoped>
.ui-form-field {
  min-width: 0;
  display: grid;
  grid-template-rows: 1.125rem auto;
  gap: var(--space-6);
  color: var(--color-muted);
  font-size: var(--font-13);
  font-weight: 600;
}

.ui-form-field.has-message {
  grid-template-rows: 1.125rem auto auto;
}

.ui-form-field.hide-label {
  grid-template-rows: auto;
}

.ui-form-field.hide-label.has-message {
  grid-template-rows: auto auto;
}

.ui-form-field-label {
  min-height: 1.125rem;
  display: flex;
  align-items: center;
  line-height: 1.125rem;
}

.ui-form-field-label.is-hidden {
  position: absolute;
  width: 0.0625rem;
  height: 0.0625rem;
  padding: 0;
  margin: -0.0625rem;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}

small {
  color: var(--color-text-muted);
  font-size: var(--font-11);
  font-weight: 500;
  line-height: 1.45;
}

small.error {
  color: var(--color-danger);
}
</style>
