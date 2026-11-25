<script setup>
import { HoverCardTrigger } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'a' },
    asChild: Boolean,
})
</script>

<template>
    <HoverCardTrigger :as="props.as" :as-child="props.asChild">
        <slot />
    </HoverCardTrigger>
</template>
