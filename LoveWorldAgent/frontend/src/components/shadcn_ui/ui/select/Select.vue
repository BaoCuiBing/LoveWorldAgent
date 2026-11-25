<script setup>
import { SelectRoot } from 'reka-ui'
const props = defineProps({
    modelValue: { type: [String, Number, Object, Array], default: undefined },
    defaultValue: { type: [String, Number, Object, Array], default: undefined },
    disabled: Boolean,
    name: { type: String, default: undefined },
    required: Boolean,
})
const emits = defineEmits(['update:modelValue'])
</script>

<template>
    <SelectRoot
        :model-value="props.modelValue"
        :default-value="props.defaultValue"
        :disabled="props.disabled"
        :name="props.name"
        :required="props.required"
        @update:model-value="emits('update:modelValue', $event)"
    >
        <slot />
    </SelectRoot>
</template>
