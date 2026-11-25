<script setup>
import { TooltipTrigger } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'button' },
    asChild: Boolean,
})
</script>

<template>
    <TooltipTrigger :as="props.as" :as-child="props.asChild">
        <slot />
    </TooltipTrigger>
</template>
