<script setup>
import { PopoverAnchor } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'div' },
    asChild: Boolean,
})
</script>

<template>
    <PopoverAnchor :as="props.as" :as-child="props.asChild">
        <slot />
    </PopoverAnchor>
</template>
