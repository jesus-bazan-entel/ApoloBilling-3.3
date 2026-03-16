/**
 * Rate Cards Management - JavaScript
 * Apolo Billing System
 */

// Toast notification helper
function showToast(message, type = 'success') {
    const toastContainer = document.getElementById('toastContainer');
    const toast = document.createElement('div');
    toast.className = `toast align-items-center text-white bg-${type} border-0`;
    toast.setAttribute('role', 'alert');
    toast.setAttribute('aria-live', 'assertive');
    toast.setAttribute('aria-atomic', 'true');
    
    toast.innerHTML = `
        <div class="d-flex">
            <div class="toast-body">${message}</div>
            <button type="button" class="btn-close btn-close-white me-2 m-auto" data-bs-dismiss="toast"></button>
        </div>
    `;
    
    toastContainer.appendChild(toast);
    const bsToast = new bootstrap.Toast(toast);
    bsToast.show();
    
    toast.addEventListener('hidden.bs.toast', () => {
        toast.remove();
    });
}

// Initialize DataTable
let rateCardsTable;
$(document).ready(function() {
    rateCardsTable = $('#rateCardsTable').DataTable({
        language: {
            url: '/static/js/Spanish.json'
        },
        order: [[1, 'asc'], [7, 'desc']], // Order by prefix, then priority
        pageLength: 25,
        responsive: true,
        columns: [
            { orderable: false }, // Actions
            null, // Prefix
            null, // Name
            null, // Rate/min
            null, // Increment
            null, // Connection Fee
            null, // Effective Start
            null, // Priority
            null  // Created At
        ]
    });
});

// Create Rate Card
document.getElementById('createRateCardForm').addEventListener('submit', async function(e) {
    e.preventDefault();
    
    const formData = {
        destination_prefix: document.getElementById('destination_prefix').value.trim(),
        destination_name: document.getElementById('destination_name').value.trim(),
        rate_per_minute: parseFloat(document.getElementById('rate_per_minute').value),
        billing_increment: parseInt(document.getElementById('billing_increment').value),
        connection_fee: parseFloat(document.getElementById('connection_fee').value) || 0.0,
        priority: parseInt(document.getElementById('priority').value) || 100
    };
    
    try {
        const response = await fetch('/api/rate-cards', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(formData)
        });
        
        const data = await response.json();
        
        if (response.ok) {
            showToast('Rate Card creada exitosamente', 'success');
            bootstrap.Modal.getInstance(document.getElementById('createRateCardModal')).hide();
            document.getElementById('createRateCardForm').reset();
            setTimeout(() => location.reload(), 1000);
        } else {
            showToast(data.detail || 'Error al crear Rate Card', 'danger');
        }
    } catch (error) {
        console.error('Error:', error);
        showToast('Error de conexión al servidor', 'danger');
    }
});

// Edit Rate Card
function editRateCard(id) {
    // Find the rate card data from the table
    const row = document.querySelector(`button[onclick="editRateCard(${id})"]`).closest('tr');
    const cells = row.querySelectorAll('td');
    
    document.getElementById('edit_rate_card_id').value = id;
    document.getElementById('edit_destination_prefix').value = cells[1].textContent.trim();
    document.getElementById('edit_destination_name').value = cells[2].textContent.trim();
    document.getElementById('edit_rate_per_minute').value = parseFloat(cells[3].textContent.replace('$', '').trim());
    document.getElementById('edit_billing_increment').value = parseInt(cells[4].textContent.replace('s', '').trim());
    document.getElementById('edit_connection_fee').value = parseFloat(cells[5].textContent.replace('$', '').trim());
    document.getElementById('edit_priority').value = parseInt(cells[7].textContent.trim());
    
    const editModal = new bootstrap.Modal(document.getElementById('editRateCardModal'));
    editModal.show();
}

// Update Rate Card
document.getElementById('editRateCardForm').addEventListener('submit', async function(e) {
    e.preventDefault();
    
    const id = document.getElementById('edit_rate_card_id').value;
    const formData = {
        destination_prefix: document.getElementById('edit_destination_prefix').value.trim(),
        destination_name: document.getElementById('edit_destination_name').value.trim(),
        rate_per_minute: parseFloat(document.getElementById('edit_rate_per_minute').value),
        billing_increment: parseInt(document.getElementById('edit_billing_increment').value),
        connection_fee: parseFloat(document.getElementById('edit_connection_fee').value) || 0.0,
        priority: parseInt(document.getElementById('edit_priority').value) || 100
    };
    
    try {
        const response = await fetch(`/api/rate-cards/${id}`, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(formData)
        });
        
        const data = await response.json();
        
        if (response.ok) {
            showToast('Rate Card actualizada exitosamente', 'success');
            bootstrap.Modal.getInstance(document.getElementById('editRateCardModal')).hide();
            setTimeout(() => location.reload(), 1000);
        } else {
            showToast(data.detail || 'Error al actualizar Rate Card', 'danger');
        }
    } catch (error) {
        console.error('Error:', error);
        showToast('Error de conexión al servidor', 'danger');
    }
});

// Delete Rate Card
async function deleteRateCard(id) {
    if (!confirm('¿Está seguro de eliminar esta Rate Card? Esta acción no se puede deshacer.')) {
        return;
    }
    
    try {
        const response = await fetch(`/api/rate-cards/${id}`, {
            method: 'DELETE'
        });
        
        if (response.ok) {
            showToast('Rate Card eliminada exitosamente', 'success');
            setTimeout(() => location.reload(), 1000);
        } else {
            const data = await response.json();
            showToast(data.detail || 'Error al eliminar Rate Card', 'danger');
        }
    } catch (error) {
        console.error('Error:', error);
        showToast('Error de conexión al servidor', 'danger');
    }
}

// Search Rate Card by Destination Number
document.getElementById('searchRateCardForm').addEventListener('submit', async function(e) {
    e.preventDefault();
    
    const destination = document.getElementById('search_destination').value.trim();
    
    if (!destination) {
        showToast('Ingrese un número de destino', 'warning');
        return;
    }
    
    try {
        const response = await fetch(`/api/rate-cards/search?destination=${encodeURIComponent(destination)}`);
        const data = await response.json();
        
        const resultDiv = document.getElementById('searchResult');
        
        if (response.ok && data.rate_card) {
            const rc = data.rate_card;
            resultDiv.innerHTML = `
                <div class="alert alert-success">
                    <h5><i class="bi bi-check-circle-fill me-2"></i>Rate Card Encontrada</h5>
                    <hr>
                    <div class="row">
                        <div class="col-md-6">
                            <p><strong>Prefijo:</strong> ${rc.destination_prefix}</p>
                            <p><strong>Destino:</strong> ${rc.destination_name}</p>
                            <p><strong>Tarifa/min:</strong> $${rc.rate_per_minute.toFixed(4)}</p>
                            <p><strong>Tarifa/seg:</strong> $${data.rate_per_second.toFixed(6)}</p>
                        </div>
                        <div class="col-md-6">
                            <p><strong>Incremento:</strong> ${rc.billing_increment}s</p>
                            <p><strong>Cargo Conexión:</strong> $${rc.connection_fee.toFixed(4)}</p>
                            <p><strong>Prioridad:</strong> ${rc.priority}</p>
                            <p><strong>Match Length:</strong> ${data.matched_length} dígitos</p>
                        </div>
                    </div>
                </div>
            `;
        } else {
            resultDiv.innerHTML = `
                <div class="alert alert-warning">
                    <i class="bi bi-exclamation-triangle-fill me-2"></i>
                    No se encontró Rate Card para el destino <strong>${destination}</strong>
                </div>
            `;
        }
    } catch (error) {
        console.error('Error:', error);
        showToast('Error al buscar Rate Card', 'danger');
    }
});

// Bulk Import Rate Cards
document.getElementById('bulkImportForm').addEventListener('submit', async function(e) {
    e.preventDefault();
    
    const fileInput = document.getElementById('csv_file');
    const file = fileInput.files[0];
    
    if (!file) {
        showToast('Seleccione un archivo CSV', 'warning');
        return;
    }
    
    const formData = new FormData();
    formData.append('file', file);
    
    // Show loading spinner
    const importBtn = document.getElementById('importBtn');
    const originalText = importBtn.innerHTML;
    importBtn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Importando...';
    importBtn.disabled = true;
    
    try {
        const response = await fetch('/api/rate-cards/bulk-import', {
            method: 'POST',
            body: formData
        });
        
        const data = await response.json();
        
        if (response.ok) {
            showToast(`Importación exitosa: ${data.imported} registros importados, ${data.skipped} omitidos`, 'success');
            bootstrap.Modal.getInstance(document.getElementById('bulkImportModal')).hide();
            document.getElementById('bulkImportForm').reset();
            setTimeout(() => location.reload(), 1500);
        } else {
            showToast(data.detail || 'Error en la importación', 'danger');
        }
    } catch (error) {
        console.error('Error:', error);
        showToast('Error de conexión al servidor', 'danger');
    } finally {
        importBtn.innerHTML = originalText;
        importBtn.disabled = false;
    }
});

// Export Rate Cards to CSV
function exportRateCards() {
    window.location.href = '/api/rate-cards/export';
}

// Filter by prefix
document.getElementById('filterPrefix').addEventListener('input', function(e) {
    rateCardsTable.column(1).search(e.target.value).draw();
});

// Filter by destination name
document.getElementById('filterDestination').addEventListener('input', function(e) {
    rateCardsTable.column(2).search(e.target.value).draw();
});
