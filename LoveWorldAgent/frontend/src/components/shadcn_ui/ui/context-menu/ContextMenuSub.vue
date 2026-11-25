<script setup>
import { ContextMenuSub } from 'reka-ui'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    defaultOpen: Boolean,
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <ContextMenuSub :open="props.open" :default-open="props.defaultOpen" @update:open="emits('update:open', $event)">
        <slot />
    </ContextMenuSub>
</template>
