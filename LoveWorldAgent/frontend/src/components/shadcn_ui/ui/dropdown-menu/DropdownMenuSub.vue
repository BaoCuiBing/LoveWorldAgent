<script setup>
import { DropdownMenuSub } from 'reka-ui'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    defaultOpen: Boolean,
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <DropdownMenuSub :open="props.open" :default-open="props.defaultOpen" @update:open="emits('update:open', $event)">
        <slot />
    </DropdownMenuSub>
</template>
