<script setup>
import { DropdownMenuTrigger } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'button' },
    asChild: Boolean,
    disabled: Boolean,
})
</script>

<template>
    <DropdownMenuTrigger :as="props.as" :as-child="props.asChild" :disabled="props.disabled">
        <slot />
    </DropdownMenuTrigger>
</template>
