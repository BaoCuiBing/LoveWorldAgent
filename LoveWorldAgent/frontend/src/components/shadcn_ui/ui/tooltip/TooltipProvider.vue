<script setup>
import { TooltipProvider } from 'reka-ui'
const props = defineProps({
    delayDuration: { type: Number, default: 300 },
    skipDelayDuration: { type: Number, default: 300 },
})
</script>

<template>
    <TooltipProvider :delay-duration="props.delayDuration" :skip-delay-duration="props.skipDelayDuration">
        <slot />
    </TooltipProvider>
</template>
