<script setup>
import { CollapsibleTrigger } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'button' },
    asChild: Boolean,
})
</script>

<template>
    <CollapsibleTrigger :as="props.as" :as-child="props.asChild">
        <slot />
    </CollapsibleTrigger>
</template>
