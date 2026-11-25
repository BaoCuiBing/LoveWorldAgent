<script setup>
import { ContextMenuGroup } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'div' },
})
</script>

<template>
    <ContextMenuGroup :as="props.as">
        <slot />
    </ContextMenuGroup>
</template>
