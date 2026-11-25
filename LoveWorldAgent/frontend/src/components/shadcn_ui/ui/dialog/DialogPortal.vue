<script setup>
import { DialogPortal } from 'reka-ui'
const props = defineProps({
    to: { type: [String, Object], default: undefined },
    disabled: Boolean,
    forceMount: Boolean,
})
</script>

<template>
    <DialogPortal :to="props.to" :disabled="props.disabled" :force-mount="props.forceMount">
        <slot />
    </DialogPortal>
</template>
