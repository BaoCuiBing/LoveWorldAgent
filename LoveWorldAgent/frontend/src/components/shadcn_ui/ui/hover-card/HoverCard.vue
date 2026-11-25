<script setup>
import { HoverCardRoot } from 'reka-ui'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    defaultOpen: Boolean,
    openDelay: { type: Number, default: 300 },
    closeDelay: { type: Number, default: 200 },
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <HoverCardRoot
        :open="props.open"
        :default-open="props.defaultOpen"
        :open-delay="props.openDelay"
        :close-delay="props.closeDelay"
        @update:open="emits('update:open', $event)"
    >
        <slot />
    </HoverCardRoot>
</template>
