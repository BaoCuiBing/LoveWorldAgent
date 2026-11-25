<script setup>
import { CollapsibleRoot } from 'reka-ui'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    defaultOpen: Boolean,
    disabled: Boolean,
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <CollapsibleRoot
        :open="props.open"
        :default-open="props.defaultOpen"
        :disabled="props.disabled"
        @update:open="emits('update:open', $event)"
    >
        <slot />
    </CollapsibleRoot>
</template>
