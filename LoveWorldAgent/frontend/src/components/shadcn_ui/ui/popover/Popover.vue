<script setup>
import { PopoverRoot } from 'reka-ui'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    defaultOpen: Boolean,
    modal: { type: Boolean, default: false },
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <PopoverRoot
        :open="props.open"
        :default-open="props.defaultOpen"
        :modal="props.modal"
        @update:open="emits('update:open', $event)"
    >
        <slot />
    </PopoverRoot>
</template>
