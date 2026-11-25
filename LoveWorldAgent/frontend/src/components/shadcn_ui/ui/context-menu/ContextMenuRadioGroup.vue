<script setup>
import { ContextMenuRadioGroup } from 'reka-ui'
const props = defineProps({
    modelValue: { type: String, default: undefined },
})
const emits = defineEmits(['update:modelValue'])
</script>

<template>
    <ContextMenuRadioGroup :model-value="props.modelValue" @update:model-value="emits('update:modelValue', $event)">
        <slot />
    </ContextMenuRadioGroup>
</template>
