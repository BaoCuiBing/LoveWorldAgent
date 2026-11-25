<script setup>
import { ContextMenuTrigger } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'span' },
    asChild: Boolean,
    disabled: Boolean,
})
</script>

<template>
    <ContextMenuTrigger :as="props.as" :as-child="props.asChild" :disabled="props.disabled">
        <slot />
    </ContextMenuTrigger>
</template>
