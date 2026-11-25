<script setup>
import { ContextMenuRoot } from 'reka-ui'
const props = defineProps({
    modal: { type: Boolean, default: true },
})
</script>

<template>
    <ContextMenuRoot :modal="props.modal">
        <slot />
    </ContextMenuRoot>
</template>
