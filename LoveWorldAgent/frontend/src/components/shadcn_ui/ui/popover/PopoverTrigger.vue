<script setup>
import { PopoverTrigger } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'button' },
    asChild: Boolean,
})
</script>

<template>
    <PopoverTrigger :as="props.as" :as-child="props.asChild">
        <slot />
    </PopoverTrigger>
</template>
