<script setup>
import { DropdownMenuRadioGroup } from 'reka-ui'
const props = defineProps({
    modelValue: { type: String, default: undefined },
})
const emits = defineEmits(['update:modelValue'])
</script>

<template>
    <DropdownMenuRadioGroup :model-value="props.modelValue" @update:model-value="emits('update:modelValue', $event)">
        <slot />
    </DropdownMenuRadioGroup>
</template>
