<?php
namespace Acme\Billing;

use DateTimeImmutable;
use RuntimeException;
use Random\Randomizer;
use Acme\Auth\Token;
use Monolog\Logger;
use Illuminate\Support\Collection;
use Acme\HttpClient\Client;
use Symfony\Component\HttpFoundation\Request;
use Unlisted\CustomPkg\Helper;

final class InvoiceService {
    public function issue(Token $token, Request $request): string {
        if ($token->value() === '') {
            throw new RuntimeException('empty');
        }
        return (new DateTimeImmutable())->format('c') . get_class($request);
    }
}
